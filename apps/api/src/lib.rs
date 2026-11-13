#![forbid(unsafe_code)]

pub mod artifacts;
pub mod lifecycle;
pub mod orchestration;
pub mod report;
pub mod runner;
pub mod storage;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
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
}

impl AppState {
    pub fn new(policy: RunPolicy, repository: Option<Arc<storage::PgRepository>>) -> Self {
        Self {
            policy: Arc::new(policy),
            repository,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/projects", get(list_projects))
        .route("/api/v1/runs", get(list_runs))
        .route("/api/v1/runs", axum::routing::post(create_run))
        .route("/api/v1/runs/{run_id}", get(get_run))
        .route(
            "/api/v1/runs/{run_id}/execute",
            axum::routing::post(execute_run),
        )
        .route(
            "/api/v1/runs/{run_id}/cancel",
            axum::routing::post(cancel_run),
        )
        .route("/api/v1/runs/{run_id}/report", get(get_report))
        .route("/api/v1/runs/{run_id}/artifacts", get(list_artifacts))
        .route(
            "/api/v1/runs/{run_id}/artifacts/{kind}",
            get(download_artifact),
        )
        .route("/api/v1/runs/preflight", axum::routing::post(preflight))
        .with_state(state)
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

async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProjectSummary>>, ApiError> {
    match state.repository {
        Some(repository) => Ok(Json(repository.list_projects().await?)),
        None => Ok(Json(Vec::new())),
    }
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
    state.policy.validate(&preflight)?;
    if !request.confirmed {
        return Err(ApiError::ConfirmationRequired);
    }
    let repository = state.repository.ok_or(ApiError::Unavailable)?;
    let scenario_hash = scenario_hash(&request.scenario);
    let id = repository
        .create_queued_run(&request, &scenario_hash)
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

async fn list_artifacts(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<Vec<ArtifactRef>>, ApiError> {
    let repository = state.repository.ok_or(ApiError::NotFound)?;
    let report = repository
        .get_report(run_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let report: RunReport =
        serde_json::from_value(report).map_err(|error| ApiError::Execution(error.to_string()))?;
    Ok(Json(report.artifacts))
}

async fn download_artifact(
    State(state): State<AppState>,
    Path((run_id, kind)): Path<(Uuid, String)>,
) -> Result<axum::response::Response, ApiError> {
    let repository = state.repository.ok_or(ApiError::NotFound)?;
    let report = repository
        .get_report(run_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let report: RunReport =
        serde_json::from_value(report).map_err(|error| ApiError::Execution(error.to_string()))?;
    let artifact = report
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == kind)
        .ok_or(ApiError::NotFound)?;
    let root = std::env::var("ARTIFACT_ROOT").unwrap_or_else(|_| "artifacts".into());
    let content =
        match artifacts::LocalArtifactStore::new(root).read(std::path::Path::new(&artifact.path)) {
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
    };
    let timeout_seconds = workload.duration_seconds + workload.think_time_seconds + 15;
    let (cancel_sender, cancel_receiver) = tokio::sync::watch::channel(false);
    state
        .cancellations
        .lock()
        .expect("cancellation registry is not poisoned")
        .insert(run_id, cancel_sender);
    if let Err(error) = runner::execute(
        &request,
        std::time::Duration::from_secs(timeout_seconds as u64),
        cancel_receiver,
    )
    .await
    {
        state
            .cancellations
            .lock()
            .expect("cancellation registry is not poisoned")
            .remove(&run_id);
        if matches!(&error, runner::RunnerError::Canceled) {
            repository.cancel_run(run_id).await?;
        } else {
            repository.fail_run(run_id, &error.to_string()).await?;
        }
        return Err(ApiError::Execution(error.to_string()));
    }
    state
        .cancellations
        .lock()
        .expect("cancellation registry is not poisoned")
        .remove(&run_id);
    let summary = std::fs::read_to_string(&request.summary_json)
        .map_err(|error| ApiError::Execution(error.to_string()))?;
    let raw = std::fs::read(&request.output_json)
        .map_err(|error| ApiError::Execution(error.to_string()))?;
    let store = artifacts::LocalArtifactStore::new(&root);
    let raw_hash = store.put(&raw_relative, &raw)?;
    let summary_hash = store.put(&summary_relative, summary.as_bytes())?;
    let preflight = PreflightRequest {
        target_origin: detail.target_origin,
        scenario,
        workload,
    };
    let mut report = report::build_report(
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
    )
    .map_err(|error| ApiError::Execution(error.to_string()))?;
    report.run_id = run_id;
    report.artifacts.push(ArtifactRef {
        kind: "k6-raw-jsonl".into(),
        path: raw_relative.display().to_string(),
        sha256: raw_hash,
    });
    let report_json =
        serde_json::to_value(&report).map_err(|error| ApiError::Execution(error.to_string()))?;
    repository
        .save_report(run_id, "completed", Some(&report_json))
        .await?;
    Ok(Json(report))
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    axum::serve(listener, router(AppState::new(policy, repository))).await?;
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
        sqlx::query("INSERT INTO environments (id, project_id, name, base_url, allowed_origins, created_at, updated_at) VALUES ($1, $2, 'local', 'https://api.canvassync.tech', '[]', $3, $3)")
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
