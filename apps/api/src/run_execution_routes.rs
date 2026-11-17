use crate::*;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub(super) async fn execute_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<RunReport>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let detail = repository
        .get_run(run_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if detail.status != "queued" {
        return Err(ApiError::Conflict);
    }
    repository.validate_execution_configuration(run_id).await?;
    let _run_permit = state
        .run_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::Conflict)?;
    let scenario: ScenarioSnapshot = serde_json::from_value(detail.scenario_snapshot.clone())
        .map_err(|error| ApiError::Execution(error.to_string()))?;
    let workload: Workload = serde_json::from_value(detail.workload.clone())
        .map_err(|error| ApiError::Execution(error.to_string()))?;
    if !repository.acquire_queued_run(run_id).await? {
        return Err(ApiError::Conflict);
    }
    repository
        .record_audit_event(
            "run.execution_started",
            "run",
            Some(run_id),
            &serde_json::json!({}),
        )
        .await?;
    let root = std::env::var("ARTIFACT_ROOT").unwrap_or_else(|_| "artifacts".into());
    let run_dir = run_id.to_string();
    let relative_dir = std::path::Path::new(&run_dir);
    let absolute_dir = std::path::Path::new(&root).join(relative_dir);
    std::fs::create_dir_all(&absolute_dir)
        .map_err(|error| ApiError::Execution(error.to_string()))?;
    let raw_relative = relative_dir.join("raw.jsonl");
    let summary_relative = relative_dir.join("summary.json");
    let request = runner::K6Request {
        script: scenario.script_path.clone().into(),
        output_json: std::path::Path::new(&root).join(&raw_relative),
        summary_json: std::path::Path::new(&root).join(&summary_relative),
        vus: workload.vus,
        duration_seconds: workload.duration_seconds,
        runtime_env: approved_runtime_env(&detail.target_origin, &workload, &scenario.secret_refs),
    };
    let timeout_seconds = workload.duration_seconds + workload.think_time_seconds + 15;
    let (cancel_sender, cancel_receiver) = tokio::sync::watch::channel(false);
    state
        .cancellations
        .lock()
        .expect("cancellation registry is not poisoned")
        .insert(run_id, cancel_sender);
    let measured_start_at = chrono::Utc::now();
    let runner_result = runner::execute(
        &request,
        std::time::Duration::from_secs(timeout_seconds as u64),
        cancel_receiver,
    )
    .await;
    let mut threshold_failure = None;
    if let Err(error) = runner_result {
        let summary_exists = matches!(
            &error,
            runner::RunnerError::NonZero(_) if request.summary_json.is_file()
        );
        if summary_exists {
            threshold_failure = Some(error.to_string());
        } else {
            state
                .cancellations
                .lock()
                .expect("cancellation registry is not poisoned")
                .remove(&run_id);
            let _ = record_artifact_if_present(
                &repository,
                std::path::Path::new(&root),
                run_id,
                "k6-raw-jsonl",
                &raw_relative,
            )
            .await;
            let _ = record_artifact_if_present(
                &repository,
                std::path::Path::new(&root),
                run_id,
                "k6-summary",
                &summary_relative,
            )
            .await;
            if matches!(&error, runner::RunnerError::Canceled) {
                repository.cancel_run(run_id).await?;
            } else {
                repository.fail_run(run_id, &error.to_string()).await?;
            }
            return Err(ApiError::Execution(error.to_string()));
        }
    }
    state
        .cancellations
        .lock()
        .expect("cancellation registry is not poisoned")
        .remove(&run_id);
    let summary = std::fs::read_to_string(&request.summary_json)
        .map_err(|error| ApiError::Execution(error.to_string()))?;
    let measured_finish_at = chrono::Utc::now();
    let raw = std::fs::read(&request.output_json)
        .map_err(|error| ApiError::Execution(error.to_string()))?;
    let bucket_ingest = timeseries::ingest_jsonl(std::io::Cursor::new(raw.as_slice()), 10, 3600)
        .map_err(|error| ApiError::Execution(error.to_string()))?;
    repository
        .replace_time_buckets(run_id, &bucket_ingest.buckets)
        .await?;
    let store = artifacts::LocalArtifactStore::new(&root);
    let raw_hash = store.put(&raw_relative, &raw)?;
    let summary_hash = store.put(&summary_relative, summary.as_bytes())?;
    repository
        .register_artifact(
            run_id,
            "k6-raw-jsonl",
            &raw_relative.display().to_string(),
            &raw_hash,
            raw.len() as i64,
            false,
        )
        .await?;
    repository
        .register_artifact(
            run_id,
            "k6-summary",
            &summary_relative.display().to_string(),
            &summary_hash,
            summary.len() as i64,
            false,
        )
        .await?;
    let preflight = PreflightRequest {
        target_origin: detail.target_origin,
        scenario,
        workload,
    };
    let mut report = report::build_report_with_timing(
        &summary,
        RunStatus::Completed,
        preflight.target_origin.clone(),
        preflight.scenario.clone(),
        preflight.workload.clone(),
        ArtifactRef {
            kind: "k6-summary".into(),
            path: summary_relative.display().to_string(),
            sha256: summary_hash,
        },
        Some(measured_start_at),
        Some(measured_finish_at),
    )
    .map_err(|error| ApiError::Execution(error.to_string()))?;
    report.run_id = run_id;
    report.artifacts.push(ArtifactRef {
        kind: "k6-raw-jsonl".into(),
        path: raw_relative.display().to_string(),
        sha256: raw_hash,
    });
    if let Some(reason) = threshold_failure {
        report.execution_outcome = "threshold_failed".into();
        report.limitations.push(format!(
            "k6 exited nonzero after producing a measured summary: {reason}"
        ));
    }
    if bucket_ingest.malformed_lines > 0 || bucket_ingest.truncated {
        report.limitations.push(format!(
            "time-series ingestion skipped {} malformed lines{}",
            bucket_ingest.malformed_lines,
            if bucket_ingest.truncated {
                " and reached its bucket limit"
            } else {
                ""
            }
        ));
    }
    let report_json =
        serde_json::to_value(&report).map_err(|error| ApiError::Execution(error.to_string()))?;
    if !repository
        .save_report(run_id, "completed", Some(&report_json))
        .await?
    {
        return Err(ApiError::Conflict);
    }
    Ok(Json(report))
}

async fn record_artifact_if_present(
    repository: &storage::PgRepository,
    root: &std::path::Path,
    run_id: Uuid,
    kind: &str,
    relative_path: &std::path::Path,
) -> Result<(), ApiError> {
    let absolute = root.join(relative_path);
    let content = match std::fs::read(&absolute) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(ApiError::Execution(error.to_string())),
    };
    let hash = format!("sha256:{:x}", Sha256::digest(&content));
    repository
        .register_artifact(
            run_id,
            kind,
            &relative_path.display().to_string(),
            &hash,
            content.len() as i64,
            true,
        )
        .await?;
    Ok(())
}

pub(super) async fn cancel_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let sender = state
        .cancellations
        .lock()
        .expect("cancellation registry is not poisoned")
        .get(&run_id)
        .cloned();
    if let Some(sender) = sender {
        sender.send(true).map_err(|_| ApiError::Conflict)?;
        repository
            .record_audit_event(
                "run.cancel_requested",
                "run",
                Some(run_id),
                &serde_json::json!({}),
            )
            .await?;
        return Ok(StatusCode::ACCEPTED);
    }
    if repository.cancel_run(run_id).await? {
        repository
            .record_audit_event("run.canceled", "run", Some(run_id), &serde_json::json!({}))
            .await?;
        return Ok(StatusCode::OK);
    }
    Err(ApiError::Conflict)
}

pub(super) async fn list_audit_events(
    State(state): State<AppState>,
) -> Result<Json<Vec<AuditEvent>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    Ok(Json(repository.list_audit_events().await?))
}

pub(super) async fn preflight(
    State(state): State<AppState>,
    Json(request): Json<PreflightRequest>,
) -> Result<Json<PreflightResponse>, ApiError> {
    let validated = state.policy.validate(&request)?;
    Ok(Json(PreflightResponse {
        approved: true,
        target_origin: validated.target_origin,
        workload: request.workload,
        scenario_hash: scenario_hash(&request.scenario),
        warnings: validated.warnings,
    }))
}

fn approved_runtime_env(
    target_origin: &str,
    workload: &Workload,
    secret_refs: &[String],
) -> Vec<(String, String)> {
    let mut values = vec![
        ("TARGET_ORIGIN".into(), target_origin.into()),
        ("VUS".into(), workload.vus.to_string()),
        ("DURATION".into(), format!("{}s", workload.duration_seconds)),
        (
            "THINK_TIME_SECONDS".into(),
            workload.think_time_seconds.to_string(),
        ),
    ];
    if let Ok(value) = std::env::var("K6_FIXTURE_MODE") {
        values.push(("K6_FIXTURE_MODE".into(), value));
    }
    for key in secret_refs {
        if let Ok(value) = std::env::var(key) {
            values.push((key.clone(), value));
        }
    }
    values
}
