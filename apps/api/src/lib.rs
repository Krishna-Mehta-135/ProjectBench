#![forbid(unsafe_code)]

pub mod artifacts;
pub mod claims;
mod claims_api;
mod claims_storage;
pub mod lifecycle;
pub mod orchestration;
pub mod publish;
pub mod report;
pub mod results;
pub mod runner;
pub mod storage;
mod storage_execution;
mod storage_projects;
mod storage_run_queries;
pub mod timeseries;
pub mod verdict;

mod api_types;
mod policy;
mod project_routes;
use project_routes::*;
mod report_types;
mod run_execution_routes;
mod run_query_routes;
pub use api_types::*;
pub use policy::*;
pub use report_types::*;
use run_execution_routes::*;
use run_query_routes::*;

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use thiserror::Error;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub policy: Arc<RunPolicy>,
    pub repository: Option<Arc<storage::PgRepository>>,
    pub cancellations: Arc<Mutex<HashMap<Uuid, tokio::sync::watch::Sender<bool>>>>,
    pub operator_token: Option<Arc<str>>,
    pub run_slots: Arc<tokio::sync::Semaphore>,
}

impl AppState {
    pub fn new(policy: RunPolicy, repository: Option<Arc<storage::PgRepository>>) -> Self {
        let max_concurrent_runs = std::env::var("METRICS_MAX_CONCURRENT_RUNS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(1)
            .clamp(1, 32);
        Self {
            policy: Arc::new(policy),
            repository,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            operator_token: None,
            run_slots: Arc::new(tokio::sync::Semaphore::new(max_concurrent_runs)),
        }
    }
}

pub fn router(state: AppState) -> Router {
    let web_origin = std::env::var("METRICS_WEB_ORIGIN")
        .unwrap_or_else(|_| "http://127.0.0.1:5173".into())
        .parse::<HeaderValue>()
        .expect("METRICS_WEB_ORIGIN must be a valid origin");
    let cors = CorsLayer::new()
        .allow_origin(web_origin)
        .allow_methods(Any)
        .allow_headers(Any);
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/v1/projects", get(list_projects))
        .route("/api/v1/claims", get(claims_api::list_claims))
        .route("/api/v1/claims/{claim_id}", get(claims_api::get_claim))
        .route(
            "/api/v1/claims/{claim_id}/results",
            get(claims_api::list_results),
        )
        .route(
            "/api/v1/claims/{claim_id}/verdict",
            get(claims_api::get_verdict),
        )
        .route(
            "/api/v1/claims/{claim_id}/verdicts",
            get(claims_api::list_verdicts),
        )
        .route(
            "/api/v1/claims/{claim_id}/publish",
            post(claims_api::publish_claim),
        )
        .route("/api/v1/results", post(claims_api::post_result))
        .route("/api/v1/projects", post(create_project))
        .route(
            "/api/v1/projects/{project_id}",
            get(get_project).patch(update_project),
        )
        .route("/api/v1/projects/{project_id}/export", get(export_project))
        .route("/api/v1/projects/import", post(import_project))
        .route(
            "/api/v1/projects/{project_id}/environments",
            get(list_environments).post(create_environment),
        )
        .route(
            "/api/v1/projects/{project_id}/environments/{environment_id}",
            patch(update_environment),
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
        .route("/api/v1/audit-events", get(list_audit_events))
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
        .route(
            "/api/v1/runs/{run_id}/report/markdown",
            get(get_markdown_report),
        )
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
        .layer(cors)
        .with_state(state)
}

async fn require_operator(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if state.operator_token.is_none() || path == "/health" || path == "/ready" {
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

async fn ready(State(state): State<AppState>) -> Response {
    let Some(repository) = state.repository else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status": "unready", "reason": "database is not configured"})),
        )
            .into_response();
    };
    match repository.ready().await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "ready"}))).into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status": "unready", "reason": "database is unavailable"})),
        )
            .into_response(),
    }
}

type MetricExtractor = fn(&RunReport) -> Option<f64>;

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
    #[error("invalid benchmark result: {0}")]
    InvalidResult(String),
    #[error(transparent)]
    Publish(#[from] publish::PublishError),
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
        } else if matches!(&self, Self::Publish(_)) {
            StatusCode::INTERNAL_SERVER_ERROR
        } else {
            StatusCode::BAD_REQUEST
        };
        (status, Json(serde_json::json!({"error": self.to_string()}))).into_response()
    }
}

pub async fn serve(addr: SocketAddr) -> anyhow::Result<()> {
    let policy = RunPolicy {
        allowed_origins: configured_allowed_origins(),
        max_vus: 10,
        max_duration_seconds: 300,
        max_runtime_seconds: 360,
    };
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let repository = match std::env::var("DATABASE_URL") {
        Ok(database_url) => {
            let repository = storage::PgRepository::connect(&database_url).await?;
            repository.sync_claims(&claims::bundled_claims()?).await?;
            Some(Arc::new(repository))
        }
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

fn configured_allowed_origins() -> Vec<String> {
    let configured = std::env::var("METRICS_ALLOWED_ORIGINS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|origins| !origins.is_empty());
    configured.unwrap_or_else(|| vec!["https://api.canvassync.tech".into()])
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
