use crate::{
    ArtifactRef, MetricSummary, RunReport, RunStatus, ScenarioSnapshot, ThresholdResult, Workload,
};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct K6Summary {
    pub metrics: K6Metrics,
}

#[derive(Debug, Deserialize)]
pub struct K6Metrics {
    pub http_reqs: K6MetricValue,
    pub http_req_failed: K6MetricValue,
    pub http_req_duration: K6Duration,
}

#[derive(Debug, Deserialize)]
pub struct K6MetricValue {
    pub value: f64,
}

#[derive(Debug, Deserialize)]
pub struct K6Duration {
    pub values: K6DurationValues,
}

#[derive(Debug, Deserialize)]
pub struct K6DurationValues {
    #[serde(rename = "p(50)")]
    pub p50: Option<f64>,
    #[serde(rename = "p(90)")]
    pub p90: Option<f64>,
    #[serde(rename = "p(95)")]
    pub p95: Option<f64>,
    #[serde(rename = "p(99)")]
    pub p99: Option<f64>,
}

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("summary JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("summary is missing required metric data")]
    MissingMetrics,
}

pub fn build_report(
    summary_json: &str,
    status: RunStatus,
    target_origin: String,
    scenario: ScenarioSnapshot,
    workload: Workload,
    artifact: ArtifactRef,
) -> Result<RunReport, ReportError> {
    build_report_with_timing(
        summary_json,
        status,
        target_origin,
        scenario,
        workload,
        artifact,
        None,
        None,
    )
}

pub fn build_report_with_timing(
    summary_json: &str,
    status: RunStatus,
    target_origin: String,
    scenario: ScenarioSnapshot,
    workload: Workload,
    artifact: ArtifactRef,
    measured_start_at: Option<chrono::DateTime<chrono::Utc>>,
    measured_finish_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<RunReport, ReportError> {
    let summary: K6Summary = serde_json::from_str(summary_json)?;
    if summary.metrics.http_reqs.value < 0.0 {
        return Err(ReportError::MissingMetrics);
    }
    let request_count = summary.metrics.http_reqs.value.round() as u64;
    let metrics = MetricSummary {
        request_count,
        throughput_per_second: request_count as f64 / workload.duration_seconds.max(1) as f64,
        error_rate: summary.metrics.http_req_failed.value,
        p50_ms: summary.metrics.http_req_duration.values.p50,
        p90_ms: summary.metrics.http_req_duration.values.p90,
        p95_ms: summary.metrics.http_req_duration.values.p95,
        p99_ms: summary.metrics.http_req_duration.values.p99,
    };
    let error_rate = metrics.error_rate;
    let passed = matches!(status, RunStatus::Completed) && error_rate < 0.05;
    let elapsed_seconds = measured_start_at
        .zip(measured_finish_at)
        .map(|(start, finish)| (finish - start).num_milliseconds().max(0) as f64 / 1000.0);
    Ok(RunReport {
        schema_version: "1.0".into(),
        run_id: Uuid::new_v4(),
        status,
        target_origin,
        scenario,
        workload,
        synthetic_traffic: true,
        metrics,
        thresholds: vec![ThresholdResult {
            name: "http_req_failed < 5%".into(),
            passed,
            observed: Some(error_rate),
            limit: Some(0.05),
        }],
        artifacts: vec![artifact],
        limitations: vec![
            "Synthetic k6 traffic does not represent real users or browser rendering.".into(),
        ],
        measured_start_at,
        measured_finish_at,
        elapsed_seconds,
        execution_outcome: "measured".into(),
    })
}

pub fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ArtifactRef;

    fn summary(error_rate: f64) -> String {
        format!(
            r#"{{"metrics":{{"http_reqs":{{"value":100}},"http_req_failed":{{"value":{error_rate}}},"http_req_duration":{{"values":{{"p(50)":10,"p(90)":20,"p(95)":30,"p(99)":40}}}}}}}}"#
        )
    }
    fn scenario() -> ScenarioSnapshot {
        ScenarioSnapshot {
            name: "local".into(),
            version: "1".into(),
            script_path: "fake.js".into(),
            routes: vec!["GET /health".into()],
        }
    }
    fn workload() -> Workload {
        Workload {
            vus: 1,
            duration_seconds: 1,
            think_time_seconds: 0,
        }
    }
    fn artifact() -> ArtifactRef {
        ArtifactRef {
            kind: "summary".into(),
            path: "summary.json".into(),
            sha256: "sha256:test".into(),
        }
    }

    #[test]
    fn successful_summary_becomes_passing_report() {
        let report = build_report(
            &summary(0.01),
            RunStatus::Completed,
            "https://local.test".into(),
            scenario(),
            workload(),
            artifact(),
        )
        .unwrap();
        assert!(report.thresholds[0].passed);
        assert_eq!(report.metrics.request_count, 100);
    }

    #[test]
    fn failed_run_cannot_become_a_passing_report() {
        let report = build_report(
            &summary(0.01),
            RunStatus::Failed,
            "https://local.test".into(),
            scenario(),
            workload(),
            artifact(),
        )
        .unwrap();
        assert!(!report.thresholds[0].passed);
    }

    #[test]
    fn timed_report_records_measured_window_and_explicit_outcome() {
        let start = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let finish = start + chrono::Duration::milliseconds(1500);
        let report = build_report_with_timing(
            &summary(0.08),
            RunStatus::Completed,
            "https://local.test".into(),
            scenario(),
            workload(),
            artifact(),
            Some(start),
            Some(finish),
        )
        .unwrap();
        assert_eq!(report.elapsed_seconds, Some(1.5));
        assert_eq!(report.measured_start_at, Some(start));
        assert_eq!(report.measured_finish_at, Some(finish));
        assert_eq!(report.execution_outcome, "measured");
        assert!(!report.thresholds[0].passed);
    }
}
