use crate::{ScenarioSnapshot, Workload};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub secret_refs: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
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
pub struct UpdateEnvironmentRequest {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateScenarioRequest {
    pub name: String,
    pub version: String,
    pub script_path: String,
    #[serde(default)]
    pub config_snapshot: serde_json::Value,
    #[serde(default)]
    pub secret_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryExport {
    pub schema_version: String,
    pub project: ProjectConfig,
    pub environments: Vec<EnvironmentConfig>,
    pub scenarios: Vec<ScenarioConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub name: String,
    pub base_url: String,
    pub allowed_origins: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioConfig {
    pub name: String,
    pub version: String,
    pub script_path: String,
    pub config_snapshot: serde_json::Value,
    pub config_sha256: String,
    #[serde(default)]
    pub secret_refs: Vec<String>,
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
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub scenario_id: Uuid,
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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: Uuid,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RunListQuery {
    pub project_id: Option<Uuid>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
