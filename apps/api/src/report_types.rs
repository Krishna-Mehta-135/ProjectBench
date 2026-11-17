use crate::{ScenarioSnapshot, Workload};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
