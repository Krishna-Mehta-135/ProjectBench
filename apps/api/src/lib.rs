#![forbid(unsafe_code)]

pub mod artifacts;
pub mod lifecycle;
pub mod orchestration;
pub mod report;
pub mod runner;
pub mod storage;
pub mod timeseries;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub policy: Arc<RunPolicy>,
    pub repository: Option<Arc<storage::PgRepository>>,
    pub cancellations: Arc<Mutex<HashMap<Uuid, tokio::sync::watch::Sender<bool>>>>,
    pub operator_token: Option<Arc<str>>,
}

impl AppState {
    pub fn new(policy: RunPolicy, repository: Option<Arc<storage::PgRepository>>) -> Self {
        Self {
            policy: Arc::new(policy),
            repository,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            operator_token: None,
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/projects", get(list_projects))
        .route("/api/v1/projects", post(create_project))
        .route("/api/v1/projects/{project_id}", get(get_project))
        .route(
            "/api/v1/projects/{project_id}/environments",
            get(list_environments).post(create_environment),
        )
        .route(
            "/api/v1/projects/{project_id}/scenarios",
            get(list_scenarios).post(create_scenario),
        )
        .route(
            "/api/v1/projects/{project_id}/baselines",
            get(get_baseline).post(select_baseline),
        )
        .route("/api/v1/runs", get(list_runs))
        .route("/api/v1/runs", axum::routing::post(create_run))
        .route("/api/v1/runs/{run_id}", get(get_run))
        .route(
            "/api/v1/runs/{baseline_id}/compare/{candidate_id}",
            get(compare_runs),
        )
        .route(
            "/api/v1/runs/{run_id}/execute",
            axum::routing::post(execute_run),
        )
        .route(
            "/api/v1/runs/{run_id}/cancel",
            axum::routing::post(cancel_run),
        )
        .route("/api/v1/runs/{run_id}/report", get(get_report))
        .route("/api/v1/runs/{run_id}/timeseries", get(get_time_series))
        .route("/api/v1/runs/{run_id}/artifacts", get(list_artifacts))
        .route(
            "/api/v1/runs/{run_id}/artifacts/{kind}",
            get(download_artifact),
        )
        .route("/api/v1/runs/preflight", axum::routing::post(preflight))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_operator,
        ))
        .with_state(state)
}

async fn require_operator(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if state.operator_token.is_none() || path == "/health" {
        return next.run(request).await;
    }
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|token| state.operator_token.as_deref() == Some(token));
    if authorized {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            Json(serde_json::json!({"error": "operator authentication required"})),
        )
            .into_response()
    }
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
    pub environment: String,
    pub latest_run_id: Option<Uuid>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProjectDetail {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct EnvironmentSummary {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub base_url: String,
    pub allowed_origins: serde_json::Value,
    pub enabled: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ScenarioSummary {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub version: String,
    pub script_path: String,
    pub config_snapshot: serde_json::Value,
    pub config_sha256: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateEnvironmentRequest {
    pub name: String,
    pub base_url: String,
    pub allowed_origins: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateScenarioRequest {
    pub name: String,
    pub version: String,
    pub script_path: String,
    #[serde(default)]
    pub config_snapshot: serde_json::Value,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BaselineSummary {
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub scenario_id: Uuid,
    pub run_id: Uuid,
    pub selected_at: chrono::DateTime<chrono::Utc>,
    pub selected_by: String,
}

#[derive(Debug, Deserialize)]
pub struct SelectBaselineRequest {
    pub environment_id: Uuid,
    pub scenario_id: Uuid,
    pub run_id: Uuid,
    #[serde(default = "default_operator")]
    pub selected_by: String,
}

fn default_operator() -> String {
    "operator".into()
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RunSummary {
    pub id: Uuid,
    pub status: String,
    pub synthetic_traffic: bool,
    pub report_available: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RunDetail {
    pub id: Uuid,
    pub status: String,
    pub target_origin: String,
    pub workload: serde_json::Value,
    pub scenario_snapshot: serde_json::Value,
    pub report: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRunRequest {
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub scenario_id: Uuid,
    pub target_origin: String,
    pub scenario: ScenarioSnapshot,
    pub workload: Workload,
    pub confirmed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatedRun {
    pub id: Uuid,
    pub status: String,
    pub scenario_hash: String,
    pub synthetic_traffic: bool,
}

#[derive(Debug, Serialize)]
pub struct CompareResponse {
    pub baseline_run_id: Uuid,
    pub candidate_run_id: Uuid,
    pub compatible: bool,
    pub incompatibility_reasons: Vec<String>,
    pub metrics: Vec<MetricDelta>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MetricDelta {
    pub name: String,
    pub baseline: Option<f64>,
    pub candidate: Option<f64>,
    pub absolute: Option<f64>,
    pub relative_percent: Option<f64>,
}

type MetricExtractor = fn(&RunReport) -> Option<f64>;

async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProjectSummary>>, ApiError> {
    match state.repository {
        Some(repository) => Ok(Json(repository.list_projects().await?)),
        None => Ok(Json(Vec::new())),
    }
}

async fn create_project(
    State(state): State<AppState>,
    Json(request): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectDetail>), ApiError> {
    validate_name(&request.name)?;
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let project = repository
        .create_project(&request.name, &request.description)
        .await?;
    Ok((StatusCode::CREATED, Json(project)))
}

async fn get_project(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ProjectDetail>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    repository
        .get_project(project_id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

async fn list_environments(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<EnvironmentSummary>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    Ok(Json(repository.list_environments(project_id).await?))
}

async fn create_environment(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateEnvironmentRequest>,
) -> Result<(StatusCode, Json<EnvironmentSummary>), ApiError> {
    validate_name(&request.name)?;
    let base_url = normalize_origin(&request.base_url)?;
    if request.allowed_origins.is_empty() {
        return Err(ApiError::Policy(PolicyError::EmptyScenario));
    }
    let mut allowed = Vec::with_capacity(request.allowed_origins.len());
    for origin in request.allowed_origins {
        allowed.push(normalize_origin(&origin)?);
    }
    if !allowed.iter().any(|origin| origin == &base_url) {
        return Err(ApiError::Policy(PolicyError::TargetNotAllowlisted));
    }
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let environment = repository
        .create_environment(project_id, &request.name, &base_url, &allowed)
        .await?;
    Ok((StatusCode::CREATED, Json(environment)))
}

async fn list_scenarios(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<ScenarioSummary>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    Ok(Json(repository.list_scenarios(project_id).await?))
}

async fn get_baseline(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<BaselineSummary>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    repository
        .get_baseline(project_id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

async fn select_baseline(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<SelectBaselineRequest>,
) -> Result<Json<BaselineSummary>, ApiError> {
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    Ok(Json(
        repository
            .select_baseline(
                project_id,
                request.environment_id,
                request.scenario_id,
                request.run_id,
                &request.selected_by,
            )
            .await?,
    ))
}

async fn create_scenario(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateScenarioRequest>,
) -> Result<(StatusCode, Json<ScenarioSummary>), ApiError> {
    validate_name(&request.name)?;
    if request.version.trim().is_empty()
        || request.script_path.trim().is_empty()
        || request.script_path.contains("..")
        || request.script_path.starts_with('/')
    {
        return Err(ApiError::Policy(PolicyError::EmptyScenario));
    }
    let config_sha256 = format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&request.config_snapshot).expect("config serializes"))
    );
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let scenario = repository
        .create_scenario(
            project_id,
            &request.name,
            &request.version,
            &request.script_path,
            &request.config_snapshot,
            &config_sha256,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(scenario)))
}

fn validate_name(name: &str) -> Result<(), ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 120 {
        return Err(ApiError::Policy(PolicyError::EmptyScenario));
    }
    Ok(())
}

fn normalize_origin(value: &str) -> Result<String, ApiError> {
    let parsed = Url::parse(value).map_err(|_| ApiError::Policy(PolicyError::InvalidOrigin))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || parsed.path() != "/"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(ApiError::Policy(PolicyError::InvalidOrigin));
    }
    Ok(parsed.origin().ascii_serialization())
}

async fn list_runs(State(state): State<AppState>) -> Result<Json<Vec<RunSummary>>, ApiError> {
    match state.repository {
        Some(repository) => Ok(Json(repository.list_runs().await?)),
        None => Ok(Json(Vec::new())),
    }
}

async fn create_run(
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

async fn get_run(
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

async fn compare_runs(
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

async fn get_report(
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

async fn get_time_series(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<Vec<timeseries::TimeBucket>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::NotFound)?;
    if repository.get_run(run_id).await?.is_none() {
        return Err(ApiError::NotFound);
    }
    Ok(Json(repository.list_time_buckets(run_id).await?))
}

async fn list_artifacts(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<Vec<ArtifactRef>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::NotFound)?;
    Ok(Json(repository.list_artifacts(run_id).await?))
}

async fn download_artifact(
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

async fn execute_run(
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
    let scenario: ScenarioSnapshot = serde_json::from_value(detail.scenario_snapshot.clone())
        .map_err(|error| ApiError::Execution(error.to_string()))?;
    let workload: Workload = serde_json::from_value(detail.workload.clone())
        .map_err(|error| ApiError::Execution(error.to_string()))?;
    if !repository.acquire_queued_run(run_id).await? {
        return Err(ApiError::Conflict);
    }
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
        runtime_env: approved_runtime_env(&detail.target_origin, &workload),
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

async fn cancel_run(
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
        return Ok(StatusCode::ACCEPTED);
    }
    if repository.cancel_run(run_id).await? {
        return Ok(StatusCode::OK);
    }
    Err(ApiError::Conflict)
}

async fn preflight(
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workload {
    pub vus: u32,
    pub duration_seconds: u32,
    pub think_time_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreflightRequest {
    pub target_origin: String,
    pub scenario: ScenarioSnapshot,
    pub workload: Workload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioSnapshot {
    pub name: String,
    pub version: String,
    pub script_path: String,
    pub routes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PreflightResponse {
    pub approved: bool,
    pub target_origin: String,
    pub workload: Workload,
    pub scenario_hash: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RunPolicy {
    pub allowed_origins: Vec<String>,
    pub max_vus: u32,
    pub max_duration_seconds: u32,
    pub max_runtime_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTarget {
    pub target_origin: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("target origin must be an absolute HTTPS URL")]
    InvalidOrigin,
    #[error("target origin is not allowlisted")]
    TargetNotAllowlisted,
    #[error("virtual users must be between 1 and {max}")]
    VusOutOfRange { max: u32 },
    #[error("duration must be between 1 and {max} seconds")]
    DurationOutOfRange { max: u32 },
    #[error("workload exceeds the hard runtime limit")]
    RuntimeLimitExceeded,
    #[error("scenario must contain at least one route")]
    EmptyScenario,
}

impl RunPolicy {
    pub fn validate(&self, request: &PreflightRequest) -> Result<ValidatedTarget, PolicyError> {
        let parsed = Url::parse(&request.target_origin).map_err(|_| PolicyError::InvalidOrigin)?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || parsed.path() != "/"
            || parsed.username() != ""
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(PolicyError::InvalidOrigin);
        }
        let normalized = parsed.origin().ascii_serialization();
        if !self
            .allowed_origins
            .iter()
            .any(|origin| origin == &normalized)
        {
            return Err(PolicyError::TargetNotAllowlisted);
        }
        if request.workload.vus == 0 || request.workload.vus > self.max_vus {
            return Err(PolicyError::VusOutOfRange { max: self.max_vus });
        }
        if request.workload.duration_seconds == 0
            || request.workload.duration_seconds > self.max_duration_seconds
        {
            return Err(PolicyError::DurationOutOfRange {
                max: self.max_duration_seconds,
            });
        }
        if request.workload.duration_seconds + request.workload.think_time_seconds
            > self.max_runtime_seconds
        {
            return Err(PolicyError::RuntimeLimitExceeded);
        }
        if request.scenario.routes.is_empty() {
            return Err(PolicyError::EmptyScenario);
        }
        Ok(ValidatedTarget {
            target_origin: normalized,
            warnings: vec![
                "Measurements represent synthetic k6 virtual users, not real users.".into(),
            ],
        })
    }
}

pub fn scenario_hash(snapshot: &ScenarioSnapshot) -> String {
    let encoded = serde_json::to_vec(snapshot).expect("scenario snapshot is serializable");
    format!("sha256:{:x}", Sha256::digest(encoded))
}

fn approved_runtime_env(target_origin: &str, workload: &Workload) -> Vec<(String, String)> {
    let mut values = vec![
        ("TARGET_ORIGIN".into(), target_origin.into()),
        ("VUS".into(), workload.vus.to_string()),
        ("DURATION".into(), format!("{}s", workload.duration_seconds)),
        (
            "THINK_TIME_SECONDS".into(),
            workload.think_time_seconds.to_string(),
        ),
    ];
    for key in [
        "TEST_ACCOUNT_EMAIL",
        "TEST_ACCOUNT_PASSWORD",
        "SEEDED_ROOM_ID",
        "K6_FIXTURE_MODE",
    ] {
        if let Ok(value) = std::env::var(key) {
            values.push((key.into(), value));
        }
    }
    values
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Canceled,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    pub schema_version: String,
    pub run_id: Uuid,
    pub status: RunStatus,
    pub target_origin: String,
    pub scenario: ScenarioSnapshot,
    pub workload: Workload,
    pub synthetic_traffic: bool,
    pub metrics: MetricSummary,
    pub thresholds: Vec<ThresholdResult>,
    pub artifacts: Vec<ArtifactRef>,
    pub limitations: Vec<String>,
    #[serde(default)]
    pub measured_start_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub measured_finish_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub elapsed_seconds: Option<f64>,
    #[serde(default = "default_execution_outcome")]
    pub execution_outcome: String,
}

fn default_execution_outcome() -> String {
    "measured".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricSummary {
    pub request_count: u64,
    pub throughput_per_second: f64,
    pub error_rate: f64,
    pub p50_ms: Option<f64>,
    pub p90_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub p99_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdResult {
    pub name: String,
    pub passed: bool,
    pub observed: Option<f64>,
    pub limit: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ArtifactRef {
    pub kind: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[error(transparent)]
    Storage(#[from] storage::StorageError),
    #[error(transparent)]
    Artifact(#[from] artifacts::ArtifactError),
    #[error("resource not found")]
    NotFound,
    #[error("operator confirmation is required")]
    ConfirmationRequired,
    #[error("run storage is unavailable")]
    Unavailable,
    #[error("run is not in a runnable state")]
    Conflict,
    #[error("run execution failed: {0}")]
    Execution(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = if matches!(&self, Self::NotFound) {
            StatusCode::NOT_FOUND
        } else if matches!(&self, Self::Unavailable) {
            StatusCode::SERVICE_UNAVAILABLE
        } else if matches!(&self, Self::ConfirmationRequired) {
            StatusCode::PRECONDITION_REQUIRED
        } else if matches!(&self, Self::Conflict) {
            StatusCode::CONFLICT
        } else if matches!(&self, Self::Execution(_)) {
            StatusCode::BAD_GATEWAY
        } else {
            StatusCode::BAD_REQUEST
        };
        (status, Json(serde_json::json!({"error": self.to_string()}))).into_response()
    }
}

pub async fn serve(addr: SocketAddr) -> anyhow::Result<()> {
    let policy = RunPolicy {
        allowed_origins: vec!["https://api.canvassync.tech".into()],
        max_vus: 10,
        max_duration_seconds: 300,
        max_runtime_seconds: 360,
    };
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let repository = match std::env::var("DATABASE_URL") {
        Ok(database_url) => Some(Arc::new(
            storage::PgRepository::connect(&database_url).await?,
        )),
        Err(_) => None,
    };
    let mut state = AppState::new(policy, repository);
    state.operator_token = std::env::var("METRICS_OPERATOR_TOKEN")
        .ok()
        .filter(|token| !token.trim().is_empty())
        .map(Arc::<str>::from);
    axum::serve(listener, router(state)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use chrono::Utc;
    use sqlx::PgPool;
    use tower::ServiceExt;

    fn policy() -> RunPolicy {
        RunPolicy {
            allowed_origins: vec!["https://api.canvassync.tech".into()],
            max_vus: 10,
            max_duration_seconds: 300,
            max_runtime_seconds: 360,
        }
    }
    fn request() -> PreflightRequest {
        PreflightRequest {
            target_origin: "https://api.canvassync.tech/".into(),
            scenario: ScenarioSnapshot {
                name: "Canvassync room read".into(),
                version: "1".into(),
                script_path: "k6/scenarios/canvassync-room-read.js".into(),
                routes: vec!["POST /api/v1/auth/signin".into()],
            },
            workload: Workload {
                vus: 1,
                duration_seconds: 60,
                think_time_seconds: 2,
            },
        }
    }

    #[test]
    fn accepts_allowlisted_https_origin_and_normalizes_it() {
        assert_eq!(
            policy().validate(&request()).unwrap().target_origin,
            "https://api.canvassync.tech"
        );
    }
    #[test]
    fn rejects_non_allowlisted_target() {
        let mut input = request();
        input.target_origin = "https://example.com/".into();
        assert!(matches!(
            policy().validate(&input),
            Err(PolicyError::TargetNotAllowlisted)
        ));
    }
    #[test]
    fn rejects_workload_over_caps() {
        let mut input = request();
        input.workload.vus = 11;
        assert!(matches!(
            policy().validate(&input),
            Err(PolicyError::VusOutOfRange { .. })
        ));
    }
    #[tokio::test]
    async fn health_endpoint_is_available() {
        let response = router(AppState::new(policy(), None))
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn project_list_is_explicitly_empty_until_persistence_is_configured() {
        let response = router(AppState::new(policy(), None))
            .oneshot(
                Request::builder()
                    .uri("/api/v1/projects")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), b"[]");
    }

    #[tokio::test]
    async fn create_run_requires_confirmation_before_storage() {
        let body = serde_json::json!({
            "project_id": Uuid::new_v4(),
            "environment_id": Uuid::new_v4(),
            "scenario_id": Uuid::new_v4(),
            "target_origin": "https://api.canvassync.tech/",
            "scenario": {"name":"read","version":"1","script_path":"scenario.js","routes":["GET /health"]},
            "workload": {"vus":1,"duration_seconds":1,"think_time_seconds":0},
            "confirmed": false
        });
        let response = router(AppState::new(policy(), None))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
    }

    async fn seed_run_dependencies(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
        let project_id = Uuid::new_v4();
        let environment_id = Uuid::new_v4();
        let scenario_id = Uuid::new_v4();
        let now = Utc::now();
        sqlx::query("INSERT INTO projects (id, name, created_at, updated_at) VALUES ($1, 'e2e project', $2, $2)")
            .bind(project_id)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO environments (id, project_id, name, base_url, allowed_origins, created_at, updated_at) VALUES ($1, $2, 'local', 'https://api.canvassync.tech', '[\"https://api.canvassync.tech\"]', $3, $3)")
            .bind(environment_id)
            .bind(project_id)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO scenarios (id, project_id, name, version, script_path, config_snapshot, config_sha256, created_at) VALUES ($1, $2, 'e2e', '1', 'scenario.js', '{}', 'sha256:e2e', $3)")
            .bind(scenario_id)
            .bind(project_id)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        (project_id, environment_id, scenario_id)
    }

    async fn create_e2e_run(
        app: &axum::Router,
        project_id: Uuid,
        environment_id: Uuid,
        scenario_id: Uuid,
    ) -> Uuid {
        let body = serde_json::json!({
            "project_id": project_id,
            "environment_id": environment_id,
            "scenario_id": scenario_id,
            "target_origin": "https://api.canvassync.tech/",
            "scenario": {"name":"e2e","version":"1","script_path":"scenario.js","routes":["GET /health"]},
            "workload": {"vus":1,"duration_seconds":1,"think_time_seconds":0},
            "confirmed": true
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice::<CreatedRun>(&body).unwrap().id
    }

    async fn execute_e2e_run(app: &axum::Router, run_id: Uuid) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/runs/{run_id}/execute"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn fake_k6_covers_success_failure_cancellation_and_artifacts(pool: PgPool) {
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/bin/k6");
        let artifact_root = std::env::temp_dir().join(format!("metrics-e2e-{}", Uuid::new_v4()));
        std::env::set_var("METRICS_K6_EXECUTABLE", fixture);
        std::env::set_var("ARTIFACT_ROOT", &artifact_root);
        let repository = Arc::new(storage::PgRepository::from_pool(pool.clone()));
        let app = router(AppState::new(policy(), Some(repository)));
        let (project_id, environment_id, scenario_id) = seed_run_dependencies(&pool).await;

        std::env::set_var("K6_FIXTURE_MODE", "success");
        let successful_run = create_e2e_run(&app, project_id, environment_id, scenario_id).await;
        let response = execute_e2e_run(&app, successful_run).await;
        assert_eq!(response.status(), StatusCode::OK);
        let report: RunReport = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(report.run_id, successful_run);
        assert!(report.synthetic_traffic);
        assert_eq!(report.artifacts.len(), 2);
        let artifact = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/runs/{successful_run}/artifacts/k6-raw-jsonl"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(artifact.status(), StatusCode::OK);
        assert!(axum::body::to_bytes(artifact.into_body(), usize::MAX)
            .await
            .unwrap()
            .windows(9)
            .any(|window| window == b"http_reqs"));

        std::env::set_var("K6_FIXTURE_MODE", "fail");
        let failed_run = create_e2e_run(&app, project_id, environment_id, scenario_id).await;
        assert_eq!(
            execute_e2e_run(&app, failed_run).await.status(),
            StatusCode::BAD_GATEWAY
        );
        let failed = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{failed_run}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let failed: RunDetail = serde_json::from_slice(
            &axum::body::to_bytes(failed.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(failed.status, "failed");
        assert!(failed.failure_reason.unwrap().contains("exit code"));
        assert!(failed.report.is_none());

        std::env::set_var("K6_FIXTURE_MODE", "sleep");
        let canceled_run = create_e2e_run(&app, project_id, environment_id, scenario_id).await;
        let executing = {
            let app = app.clone();
            tokio::spawn(async move { execute_e2e_run(&app, canceled_run).await })
        };
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        let cancel = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/runs/{canceled_run}/cancel"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(matches!(
            cancel.status(),
            StatusCode::ACCEPTED | StatusCode::OK
        ));
        assert_eq!(executing.await.unwrap().status(), StatusCode::BAD_GATEWAY);
        let canceled = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/runs/{canceled_run}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let canceled: RunDetail = serde_json::from_slice(
            &axum::body::to_bytes(canceled.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(canceled.status, "canceled");
        assert_eq!(
            canceled.failure_reason.as_deref(),
            Some("canceled by operator")
        );
        assert!(canceled.report.is_none());
        let _ = std::fs::remove_dir_all(artifact_root);
    }
}
