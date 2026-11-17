use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

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
    #[serde(default)]
    pub secret_refs: Vec<String>,
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
