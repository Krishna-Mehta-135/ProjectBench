use crate::claims::Claim;
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultDocument {
    pub schema_version: String,
    pub claim_id: String,
    pub project: String,
    pub variant: String,
    pub workload: String,
    pub repetition: u32,
    pub started_at: String,
    pub duration_seconds: f64,
    pub metrics: Vec<ResultMetric>,
    pub environment: ResultEnvironment,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultMetric {
    pub name: String,
    pub unit: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultEnvironment {
    pub commit: String,
    pub images: BTreeMap<String, String>,
    pub host_cpu: String,
    pub sut_cpuset: String,
    pub sut_memory_mb: u32,
    pub generator_cpu_peak_percent: f64,
    pub docker: String,
    pub os: String,
}

#[derive(Debug, Error)]
pub enum ResultError {
    #[error("result schema v1 is invalid: {0}")]
    Invalid(String),
}

impl ResultDocument {
    pub fn validate(&self, claim: &Claim) -> Result<(), ResultError> {
        let invalid = |reason: &str| ResultError::Invalid(reason.to_owned());
        if self.schema_version != "1" {
            return Err(invalid("schema_version must be 1"));
        }
        if self.claim_id != claim.id || self.project != claim.project {
            return Err(invalid("claim_id and project must match the ledger"));
        }
        if self.variant.trim().is_empty() || self.workload.trim().is_empty() {
            return Err(invalid("variant and workload are required"));
        }
        if !claim
            .workloads
            .iter()
            .any(|workload| workload == &self.workload)
        {
            return Err(invalid("workload is not listed for this claim"));
        }
        if self.repetition == 0 || !self.duration_seconds.is_finite() || self.duration_seconds < 0.0
        {
            return Err(invalid("repetition or duration is invalid"));
        }
        if DateTime::<FixedOffset>::parse_from_rfc3339(&self.started_at).is_err() {
            return Err(invalid("started_at must be RFC 3339 with a timezone"));
        }
        if self.metrics.is_empty()
            || self.metrics.iter().any(|metric| {
                metric.name.trim().is_empty()
                    || metric.unit.trim().is_empty()
                    || !metric.value.is_finite()
            })
            || !self
                .metrics
                .iter()
                .any(|metric| metric.name == claim.metric)
        {
            return Err(invalid(
                "metrics must include the claim metric with finite values",
            ));
        }
        let environment = &self.environment;
        if environment.commit.trim().is_empty()
            || environment.host_cpu.trim().is_empty()
            || environment.sut_cpuset.trim().is_empty()
            || environment.docker.trim().is_empty()
            || environment.os.trim().is_empty()
            || environment.sut_memory_mb == 0
            || !environment.generator_cpu_peak_percent.is_finite()
            || !(0.0..=100.0).contains(&environment.generator_cpu_peak_percent)
            || environment.images.is_empty()
        {
            return Err(invalid("environment is incomplete or invalid"));
        }
        if environment.images.iter().any(|(name, digest)| {
            name.trim().is_empty()
                || digest.len() != 71
                || !digest.starts_with("sha256:")
                || !digest[7..]
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        }) {
            return Err(invalid("images must contain lowercase sha256 digests"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim() -> Claim {
        crate::claims::bundled_claims()
            .expect("ledger")
            .into_iter()
            .find(|claim| claim.id == "canvassync-redis-cold-start-latency")
            .expect("cold-start claim")
    }

    fn valid_json() -> serde_json::Value {
        serde_json::json!({
            "schema_version":"1","claim_id":"canvassync-redis-cold-start-latency",
            "project":"canvassync","variant":"baseline","workload":"cold-start-100-shapes",
            "repetition":1,"started_at":"2026-11-17T10:00:00Z","duration_seconds":0.01,
            "metrics":[{"name":"join_sync_latency_ms","unit":"milliseconds","value":10.0}],
            "environment":{"commit":"abc123","images":{"ws":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"host_cpu":"Apple M5","sut_cpuset":"0-1","sut_memory_mb":384,"generator_cpu_peak_percent":40.0,"docker":"29.9.3","os":"Darwin"},
            "limitations":[]
        })
    }

    #[test]
    fn valid_result_matches_ledger() {
        let result: ResultDocument = serde_json::from_value(valid_json()).expect("schema");
        result.validate(&claim()).expect("valid result");
    }

    #[test]
    fn unknown_fields_and_invalid_provenance_are_rejected() {
        let mut unknown = valid_json();
        unknown["secret"] = serde_json::json!("do not store");
        assert!(serde_json::from_value::<ResultDocument>(unknown).is_err());
        let mut bad_digest = valid_json();
        bad_digest["environment"]["images"]["ws"] = serde_json::json!("container-id");
        let result: ResultDocument = serde_json::from_value(bad_digest).expect("shape");
        assert!(result.validate(&claim()).is_err());
    }

    #[test]
    fn claim_metric_and_workload_must_match() {
        let mut wrong = valid_json();
        wrong["workload"] = serde_json::json!("unlisted");
        let result: ResultDocument = serde_json::from_value(wrong).expect("shape");
        assert!(result.validate(&claim()).is_err());
        let mut wrong = valid_json();
        wrong["metrics"][0]["name"] = serde_json::json!("other");
        let result: ResultDocument = serde_json::from_value(wrong).expect("shape");
        assert!(result.validate(&claim()).is_err());
    }
}
