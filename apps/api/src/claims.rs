use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub id: String,
    pub project: String,
    pub source: String,
    pub source_ref: String,
    pub statement: String,
    pub kind: String,
    pub metric: String,
    pub metric_definition: String,
    pub direction: String,
    pub baseline: String,
    pub candidate: String,
    pub workloads: Vec<String>,
    pub repetitions: u32,
    pub status: String,
    pub verification: String,
    #[serde(default)]
    pub proprietary: bool,
    #[serde(default)]
    pub tolerance_percent: Option<f64>,
}

#[derive(Debug, Error)]
pub enum ClaimError {
    #[error("claims YAML is invalid: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("claim ledger is invalid: {0}")]
    Invalid(String),
}

pub fn parse_claims(yaml: &str) -> Result<Vec<Claim>, ClaimError> {
    let claims: Vec<Claim> = serde_yaml::from_str(yaml)?;
    let mut seen = HashSet::new();
    for claim in &claims {
        if claim.id.trim().is_empty() || !seen.insert(&claim.id) {
            return Err(ClaimError::Invalid(format!(
                "empty or duplicate ID: {}",
                claim.id
            )));
        }
        if claim.project.trim().is_empty()
            || claim.metric.trim().is_empty()
            || claim.workloads.is_empty()
            || claim.repetitions == 0
        {
            return Err(ClaimError::Invalid(format!(
                "missing experiment definition: {}",
                claim.id
            )));
        }
        if claim
            .tolerance_percent
            .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(ClaimError::Invalid(format!(
                "invalid tolerance: {}",
                claim.id
            )));
        }
    }
    Ok(claims)
}

pub fn bundled_claims() -> Result<Vec<Claim>, ClaimError> {
    parse_claims(include_str!("../../../claims/claims.yaml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_ledger_loads_without_duplicate_ids() {
        let claims = bundled_claims().expect("bundled ledger parses");
        assert!(claims
            .iter()
            .any(|claim| claim.id == "canvassync-redis-cold-start-latency"));
        assert!(claims
            .iter()
            .any(|claim| claim.id == "knowdex-coalesce-volume"));
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let claim = "- id: same\n  project: p\n  source: docs\n  source_ref: x\n  statement: x\n  kind: behavior\n  metric: m\n  metric_definition: x\n  direction: exact_match\n  baseline: x\n  candidate: x\n  workloads: [w]\n  repetitions: 1\n  status: unverified\n  verification: x\n";
        assert!(matches!(
            parse_claims(&format!("{claim}{claim}")),
            Err(ClaimError::Invalid(_))
        ));
    }
}
