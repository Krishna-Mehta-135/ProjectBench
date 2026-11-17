use crate::*;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

pub(super) async fn list_runs(
    State(state): State<AppState>,
    Query(query): Query<RunListQuery>,
) -> Result<Json<Vec<RunSummary>>, ApiError> {
    const DEFAULT_LIMIT: u32 = 50;
    const MAX_LIMIT: u32 = 100;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        return Err(ApiError::Execution(
            "limit must be between 1 and 100".into(),
        ));
    }
    if let Some(status) = &query.status {
        if !matches!(
            status.as_str(),
            "queued" | "running" | "completed" | "failed" | "canceled" | "aborted"
        ) {
            return Err(ApiError::Execution("invalid run status filter".into()));
        }
    }
    let offset = query.offset.unwrap_or(0);
    match state.repository {
        Some(repository) => Ok(Json(
            repository
                .list_runs(
                    query.project_id,
                    query.status.as_deref(),
                    i64::from(limit),
                    i64::from(offset),
                )
                .await?,
        )),
        None => Ok(Json(Vec::new())),
    }
}

pub(super) async fn create_run(
    State(state): State<AppState>,
    Json(request): Json<CreateRunRequest>,
) -> Result<(StatusCode, Json<CreatedRun>), ApiError> {
    let preflight = PreflightRequest {
        target_origin: request.target_origin.clone(),
        scenario: request.scenario.clone(),
        workload: request.workload.clone(),
    };
    let validated = state.policy.validate(&preflight)?;
    if !request.confirmed {
        return Err(ApiError::ConfirmationRequired);
    }
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    repository
        .validate_run_configuration(&request, &validated.target_origin)
        .await?;
    let scenario_hash = scenario_hash(&request.scenario);
    let id = repository
        .create_queued_run(&request, &validated.target_origin, &scenario_hash)
        .await?;
    repository
        .record_audit_event(
            "run.created",
            "run",
            Some(id),
            &serde_json::json!({"status": "queued"}),
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedRun {
            id,
            status: "queued".into(),
            scenario_hash,
            synthetic_traffic: true,
        }),
    ))
}

pub(super) async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<RunDetail>, ApiError> {
    let repository = state.repository.ok_or(ApiError::NotFound)?;
    repository
        .get_run(run_id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

pub(super) async fn compare_runs(
    State(state): State<AppState>,
    Path((baseline_id, candidate_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CompareResponse>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let (baseline, candidate) = repository
        .get_compare_runs(baseline_id, candidate_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let mut reasons = Vec::new();
    if baseline.status != "completed" {
        reasons.push("baseline run is not completed".into());
    }
    if candidate.status != "completed" {
        reasons.push("candidate run is not completed".into());
    }
    if baseline.project_id != candidate.project_id {
        reasons.push("runs belong to different projects".into());
    }
    if baseline.environment_id != candidate.environment_id {
        reasons.push("runs belong to different environments".into());
    }
    if baseline.scenario_id != candidate.scenario_id
        || baseline.scenario_sha256 != candidate.scenario_sha256
    {
        reasons.push("runs use different scenario versions".into());
    }
    if baseline.target_origin != candidate.target_origin {
        reasons.push("runs target different origins".into());
    }
    if baseline.workload != candidate.workload {
        reasons.push("runs use different workloads".into());
    }
    let baseline_report = baseline
        .report
        .as_ref()
        .and_then(|value| serde_json::from_value::<RunReport>(value.clone()).ok());
    let candidate_report = candidate
        .report
        .as_ref()
        .and_then(|value| serde_json::from_value::<RunReport>(value.clone()).ok());
    let metric_names: [(&str, MetricExtractor); 6] = [
        ("p50_ms", |report: &RunReport| report.metrics.p50_ms),
        ("p90_ms", |report: &RunReport| report.metrics.p90_ms),
        ("p95_ms", |report: &RunReport| report.metrics.p95_ms),
        ("p99_ms", |report: &RunReport| report.metrics.p99_ms),
        ("throughput_per_second", |report: &RunReport| {
            Some(report.metrics.throughput_per_second)
        }),
        ("error_rate", |report: &RunReport| {
            Some(report.metrics.error_rate)
        }),
    ];
    let metrics = metric_names
        .into_iter()
        .map(|(name, value)| {
            let base = baseline_report.as_ref().and_then(value);
            let candidate_value = candidate_report.as_ref().and_then(value);
            let absolute = base
                .zip(candidate_value)
                .map(|(base, candidate)| candidate - base);
            let relative_percent = base.zip(candidate_value).and_then(|(base, candidate)| {
                (base != 0.0).then_some((candidate - base) / base * 100.0)
            });
            MetricDelta {
                name: name.into(),
                baseline: base,
                candidate: candidate_value,
                absolute,
                relative_percent,
            }
        })
        .collect();
    let mut limitations = vec![
        "Deltas are descriptive and do not establish statistical significance.".into(),
        "Missing metrics remain null; a zero baseline has no relative percentage.".into(),
    ];
    if baseline_report.is_none() || candidate_report.is_none() {
        reasons.push("both runs must contain readable reports".into());
        limitations
            .push("At least one report could not be decoded as the current report schema.".into());
    }
    Ok(Json(CompareResponse {
        baseline_run_id: baseline_id,
        candidate_run_id: candidate_id,
        compatible: reasons.is_empty(),
        incompatibility_reasons: reasons,
        metrics,
        limitations,
    }))
}

pub(super) async fn get_report(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repository = state.repository.ok_or(ApiError::NotFound)?;
    repository
        .get_report(run_id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

pub(super) async fn get_markdown_report(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let repository = state.repository.ok_or(ApiError::NotFound)?;
    let value = repository
        .get_report(run_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let report: RunReport = serde_json::from_value(value)
        .map_err(|error| ApiError::Execution(format!("stored report is invalid: {error}")))?;
    let metric = |value: Option<f64>| {
        value
            .map(|value| format!("{value:.3}"))
            .unwrap_or_else(|| "unavailable".into())
    };
    let mut body = format!(
        "# Metrics run report\n\n- Run ID: `{}`\n- Schema: `{}`\n- Status: `{}`\n- Outcome: `{}`\n- Synthetic traffic: `{}`\n- Target origin: `{}`\n- Scenario: `{}` `{}`\n- Script: `{}`\n- Workload: `{}` VUs for `{}` seconds, `{}` seconds think time\n- Measured start: `{}`\n- Measured finish: `{}`\n- Elapsed seconds: `{}`\n\n## Metrics\n\n| Metric | Value |\n|---|---:|\n| Requests | {} |\n| Throughput/second | {:.3} |\n| Error rate | {:.4} |\n| p50 ms | {} |\n| p90 ms | {} |\n| p95 ms | {} |\n| p99 ms | {} |\n\n## Thresholds\n",
        report.run_id,
        report.schema_version,
        format!("{:?}", report.status).to_lowercase(),
        report.execution_outcome,
        report.synthetic_traffic,
        report.target_origin,
        report.scenario.name,
        report.scenario.version,
        report.scenario.script_path,
        report.workload.vus,
        report.workload.duration_seconds,
        report.workload.think_time_seconds,
        report
            .measured_start_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "unavailable".into()),
        report
            .measured_finish_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "unavailable".into()),
        report
            .elapsed_seconds
            .map(|value| format!("{value:.3}"))
            .unwrap_or_else(|| "unavailable".into()),
        report.metrics.request_count,
        report.metrics.throughput_per_second,
        report.metrics.error_rate,
        metric(report.metrics.p50_ms),
        metric(report.metrics.p90_ms),
        metric(report.metrics.p95_ms),
        metric(report.metrics.p99_ms),
    );
    for threshold in &report.thresholds {
        body.push_str(&format!(
            "\n- `{}`: {} (observed {}, limit {})",
            threshold.name,
            if threshold.passed { "passed" } else { "failed" },
            threshold
                .observed
                .map(|value| format!("{value:.4}"))
                .unwrap_or_else(|| "unavailable".into()),
            threshold
                .limit
                .map(|value| format!("{value:.4}"))
                .unwrap_or_else(|| "unavailable".into())
        ));
    }
    body.push_str("\n\n## Artifact references\n");
    for artifact in &report.artifacts {
        body.push_str(&format!("\n- `{}` — `{}`", artifact.kind, artifact.sha256));
    }
    body.push_str("\n\n## Limitations\n");
    for limitation in &report.limitations {
        body.push_str(&format!("\n- {limitation}"));
    }
    body.push('\n');
    let mut response = Body::from(body).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/markdown; charset=utf-8"),
    );
    Ok(response)
}

pub(super) async fn get_time_series(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<Vec<timeseries::TimeBucket>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::NotFound)?;
    if repository.get_run(run_id).await?.is_none() {
        return Err(ApiError::NotFound);
    }
    Ok(Json(repository.list_time_buckets(run_id).await?))
}

pub(super) async fn list_artifacts(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<Vec<ArtifactRef>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::NotFound)?;
    Ok(Json(repository.list_artifacts(run_id).await?))
}

pub(super) async fn download_artifact(
    State(state): State<AppState>,
    Path((run_id, kind)): Path<(Uuid, String)>,
) -> Result<axum::response::Response, ApiError> {
    let repository = state.repository.ok_or(ApiError::NotFound)?;
    let artifact = repository
        .get_artifact(run_id, &kind)
        .await?
        .ok_or(ApiError::NotFound)?;
    let root = std::env::var("ARTIFACT_ROOT").unwrap_or_else(|_| "artifacts".into());
    let content = match artifacts::LocalArtifactStore::new(root)
        .read_verified(std::path::Path::new(&artifact.path), &artifact.sha256)
    {
        Ok(content) => content,
        Err(artifacts::ArtifactError::Io(error))
            if error.kind() == std::io::ErrorKind::NotFound =>
        {
            return Err(ApiError::NotFound);
        }
        Err(error) => return Err(ApiError::Artifact(error)),
    };
    let content_type = if artifact.kind.ends_with("json") || artifact.kind == "k6-summary" {
        "application/json"
    } else {
        "application/x-ndjson"
    };
    let mut response = Body::from(content).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    Ok(response)
}
