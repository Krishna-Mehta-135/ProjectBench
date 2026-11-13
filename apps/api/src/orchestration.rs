use crate::{lifecycle, report, ArtifactRef, PreflightRequest, RunPolicy, RunReport, RunStatus};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OrchestrationError {
    #[error("operator confirmation is required")]
    ConfirmationRequired,
    #[error("run is not allowed: {0}")]
    NotAllowed(String),
    #[error("invalid state transition: {0}")]
    InvalidTransition(String),
    #[error("report construction failed: {0}")]
    Report(String),
}

pub fn confirm_and_queue(
    policy: &RunPolicy,
    request: &PreflightRequest,
    confirmed: bool,
) -> Result<RunStatus, OrchestrationError> {
    policy
        .validate(request)
        .map_err(|error| OrchestrationError::NotAllowed(error.to_string()))?;
    if !confirmed {
        return Err(OrchestrationError::ConfirmationRequired);
    }
    Ok(RunStatus::Queued)
}

pub fn advance(from: RunStatus, to: RunStatus) -> Result<RunStatus, OrchestrationError> {
    lifecycle::transition(from, to)
        .map_err(|error| OrchestrationError::InvalidTransition(error.to_string()))
}

pub fn ingest_report(
    summary: &str,
    status: RunStatus,
    request: &PreflightRequest,
    artifact: ArtifactRef,
) -> Result<RunReport, OrchestrationError> {
    report::build_report(
        summary,
        status,
        request.target_origin.clone(),
        request.scenario.clone(),
        request.workload.clone(),
        artifact,
    )
    .map_err(|error| OrchestrationError::Report(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScenarioSnapshot, Workload};

    fn policy() -> RunPolicy {
        RunPolicy {
            allowed_origins: vec!["https://local.test".into()],
            max_vus: 1,
            max_duration_seconds: 10,
            max_runtime_seconds: 10,
        }
    }
    fn request() -> PreflightRequest {
        PreflightRequest {
            target_origin: "https://local.test/".into(),
            scenario: ScenarioSnapshot {
                name: "fake".into(),
                version: "1".into(),
                script_path: "fake.js".into(),
                routes: vec!["GET /health".into()],
            },
            workload: Workload {
                vus: 1,
                duration_seconds: 1,
                think_time_seconds: 0,
            },
        }
    }

    #[test]
    fn confirmation_is_a_required_boundary() {
        assert_eq!(
            confirm_and_queue(&policy(), &request(), false),
            Err(OrchestrationError::ConfirmationRequired)
        );
        assert_eq!(
            confirm_and_queue(&policy(), &request(), true),
            Ok(RunStatus::Queued)
        );
    }
}
