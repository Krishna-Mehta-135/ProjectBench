#![forbid(unsafe_code)]

pub mod artifacts;
pub mod lifecycle;
pub mod runner;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{net::SocketAddr, sync::Arc};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub policy: Arc<RunPolicy>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/runs/preflight", axum::routing::post(preflight))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
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
        if parsed.scheme() != "https" || parsed.host_str().is_none() || parsed.path() != "/" {
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
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": self.to_string()})),
        )
            .into_response()
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
    axum::serve(
        listener,
        router(AppState {
            policy: Arc::new(policy),
        }),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
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
        let response = router(AppState {
            policy: Arc::new(policy()),
        })
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
}
