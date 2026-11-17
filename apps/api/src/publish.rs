use crate::{
    claims::Claim,
    results::ResultDocument,
    verdict::{Outcome, Summary, Verdict},
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("evidence file operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("evidence JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("evidence cannot be built: {0}")]
    Invalid(String),
}

#[derive(Debug, Serialize)]
pub struct EvidenceClaim<'a> {
    pub id: &'a str,
    pub project: &'a str,
    pub statement: &'a str,
    pub metric: &'a str,
    pub metric_definition: &'a str,
}

#[derive(Debug, Serialize)]
pub struct EvidenceWorkload<'a> {
    pub name: &'a str,
    pub outcome: Outcome,
    pub baseline: &'a Option<Summary>,
    pub candidate: &'a Option<Summary>,
    pub relative_change_percent: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct EvidenceSample<'a> {
    pub variant: &'a str,
    pub workload: &'a str,
    pub repetition: u32,
    pub value: f64,
    pub started_at: &'a str,
}

#[derive(Debug, Serialize)]
pub struct EvidenceEnvironment<'a> {
    pub commit: &'a str,
    pub images: &'a std::collections::BTreeMap<String, String>,
    pub host_cpu: &'a str,
    pub sut_cpuset: &'a str,
    pub sut_memory_mb: u32,
    pub generator_cpu_peak_percent: f64,
    pub docker: &'a str,
    pub os: &'a str,
}

#[derive(Debug, Serialize)]
pub struct EvidenceDocument<'a> {
    pub schema_version: &'static str,
    pub claim: EvidenceClaim<'a>,
    pub outcome: Outcome,
    pub tolerance_percent: Option<f64>,
    pub reproduce_command: Option<&'a str>,
    pub workloads: Vec<EvidenceWorkload<'a>>,
    pub samples: Vec<EvidenceSample<'a>>,
    pub environment: EvidenceEnvironment<'a>,
    pub limitations: Vec<&'static str>,
}

pub fn build_evidence<'a>(
    claim: &'a Claim,
    verdict: &'a Verdict,
    results: &'a [ResultDocument],
) -> Result<EvidenceDocument<'a>, PublishError> {
    let first = results
        .first()
        .ok_or_else(|| PublishError::Invalid("result set is empty".into()))?;
    if results
        .iter()
        .any(|result| result.claim_id != claim.id || result.project != claim.project)
    {
        return Err(PublishError::Invalid(
            "result set does not match claim".into(),
        ));
    }
    let samples = results
        .iter()
        .map(|result| {
            let metric = result
                .metrics
                .iter()
                .find(|metric| metric.name == claim.metric)
                .ok_or_else(|| PublishError::Invalid("claim metric missing".into()))?;
            Ok(EvidenceSample {
                variant: &result.variant,
                workload: &result.workload,
                repetition: result.repetition,
                value: metric.value,
                started_at: &result.started_at,
            })
        })
        .collect::<Result<Vec<_>, PublishError>>()?;
    let environment = &first.environment;
    Ok(EvidenceDocument {
        schema_version: "1",
        claim: EvidenceClaim {
            id: &claim.id,
            project: &claim.project,
            statement: &claim.statement,
            metric: &claim.metric,
            metric_definition: &claim.metric_definition,
        },
        outcome: verdict.outcome,
        tolerance_percent: verdict.tolerance_percent,
        reproduce_command: claim.reproduce_command.as_deref(),
        workloads: verdict
            .workloads
            .iter()
            .map(|workload| EvidenceWorkload {
                name: &workload.workload,
                outcome: workload.outcome,
                baseline: &workload.baseline,
                candidate: &workload.candidate,
                relative_change_percent: workload.relative_change_percent,
            })
            .collect(),
        samples,
        environment: EvidenceEnvironment {
            commit: &environment.commit,
            images: &environment.images,
            host_cpu: &environment.host_cpu,
            sut_cpuset: &environment.sut_cpuset,
            sut_memory_mb: environment.sut_memory_mb,
            generator_cpu_peak_percent: environment.generator_cpu_peak_percent,
            docker: &environment.docker,
            os: &environment.os,
        },
        limitations: {
            let mut limits =
                vec!["Self-measured on local Docker Compose; client and server share a host."];
            if environment.os.contains("Darwin") || environment.os.contains("macOS") {
                limits.push("Docker on macOS runs in a VM.");
            }
            if environment.host_cpu.contains("Apple") || environment.host_cpu.contains("ARM") {
                limits.push("This laptop uses ARM hardware.");
            }
            limits.push("Absolute times do not represent production latency.");
            limits
        },
    })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceIndex {
    schema_version: String,
    claims: Vec<EvidenceIndexEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceIndexEntry {
    id: String,
    project: String,
    outcome: Outcome,
    date: String,
}

pub fn write_evidence(
    root: &Path,
    claim: &Claim,
    verdict: &Verdict,
    results: &[ResultDocument],
) -> Result<(), PublishError> {
    let document = build_evidence(claim, verdict, results)?;
    let project_dir = root.join(&claim.project);
    fs::create_dir_all(&project_dir)?;
    let claim_path = project_dir.join(format!("{}.json", claim.id));
    let claim_temp = project_dir.join(format!("{}.json.tmp", claim.id));
    fs::write(&claim_temp, serde_json::to_vec_pretty(&document)?)?;
    fs::rename(claim_temp, claim_path)?;

    let index_path = root.join("index.json");
    let mut index: EvidenceIndex = if index_path.exists() {
        serde_json::from_slice(&fs::read(&index_path)?)?
    } else {
        EvidenceIndex {
            schema_version: "1".into(),
            claims: Vec::new(),
        }
    };
    let date = results
        .iter()
        .map(|result| result.started_at.as_str())
        .max()
        .unwrap_or_default();
    index.claims.retain(|entry| entry.id != claim.id);
    index.claims.push(EvidenceIndexEntry {
        id: claim.id.clone(),
        project: claim.project.clone(),
        outcome: verdict.outcome,
        date: date.into(),
    });
    index
        .claims
        .sort_by(|left, right| (&left.project, &left.id).cmp(&(&right.project, &right.id)));
    let index_temp = root.join("index.json.tmp");
    fs::write(&index_temp, serde_json::to_vec_pretty(&index)?)?;
    fs::rename(index_temp, index_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_allowlist_excludes_operator_and_private_fields() {
        let mut claim = crate::claims::bundled_claims()
            .expect("ledger")
            .into_iter()
            .find(|claim| claim.id == "canvassync-redis-cold-start-latency")
            .expect("claim");
        claim.source_ref = "/private/owner/path".into();
        claim.verification = "secret verification text".into();
        let result: ResultDocument = serde_json::from_value(serde_json::json!({
            "schema_version":"1","claim_id":claim.id,"project":claim.project,"variant":"baseline",
            "workload":"cold-start-100-shapes","repetition":1,"started_at":"2026-11-17T10:00:00Z",
            "duration_seconds":0.01,"metrics":[{"name":"join_sync_latency_ms","unit":"milliseconds","value":10.0},{"name":"private_metric","unit":"x","value":9.0}],
            "environment":{"commit":"abc","images":{"ws":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"host_cpu":"Apple M5","sut_cpuset":"0-1","sut_memory_mb":384,"generator_cpu_peak_percent":30.0,"docker":"29.9.3","os":"Darwin"},
            "limitations":["/private/other/path"]
        })).expect("result");
        let verdict = crate::verdict::evaluate(&claim, std::slice::from_ref(&result));
        let published =
            serde_json::to_string(&build_evidence(&claim, &verdict, &[result]).expect("evidence"))
                .expect("JSON");
        for forbidden in [
            "source_ref",
            "verification",
            "private_metric",
            "/private/",
            "secret",
            "next_step",
            "suggested_wording",
            "result_set_id",
        ] {
            assert!(!published.contains(forbidden), "published {forbidden}");
        }
    }

    #[test]
    fn publish_writes_claim_page_and_index() {
        let claim = crate::claims::bundled_claims()
            .expect("ledger")
            .into_iter()
            .find(|claim| claim.id == "canvassync-redis-cold-start-latency")
            .expect("claim");
        let result: ResultDocument = serde_json::from_value(serde_json::json!({
            "schema_version":"1","claim_id":claim.id,"project":claim.project,"variant":"baseline",
            "workload":"cold-start-100-shapes","repetition":1,"started_at":"2026-11-17T10:00:00Z",
            "duration_seconds":0.01,"metrics":[{"name":"join_sync_latency_ms","unit":"milliseconds","value":10.0}],
            "environment":{"commit":"abc","images":{"ws":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"host_cpu":"Apple M5","sut_cpuset":"0-1","sut_memory_mb":384,"generator_cpu_peak_percent":30.0,"docker":"29.9.3","os":"Darwin"},
            "limitations":[]
        })).expect("result");
        let results = vec![result];
        let verdict = crate::verdict::evaluate(&claim, &results);
        let root = std::env::temp_dir().join(format!(
            "projectbench-publish-test-{}",
            uuid::Uuid::new_v4()
        ));
        write_evidence(&root, &claim, &verdict, &results).expect("publish");
        let page: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("canvassync").join(format!("{}.json", claim.id))).expect("page"),
        )
        .expect("page JSON");
        let index: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("index.json")).expect("index"))
                .expect("index JSON");
        assert_eq!(page["claim"]["id"], claim.id);
        assert_eq!(page["samples"][0]["value"], 10.0);
        assert_eq!(index["claims"][0]["id"], claim.id);
        fs::remove_dir_all(root).expect("cleanup test files");
    }
}
