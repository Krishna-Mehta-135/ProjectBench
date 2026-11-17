use super::*;
use crate::results::{ResultEnvironment, ResultMetric};
use std::collections::BTreeMap;

fn claim() -> Claim {
    let mut claim = crate::claims::bundled_claims()
        .expect("ledger")
        .into_iter()
        .find(|claim| claim.id == "canvassync-redis-cold-start-latency")
        .expect("claim");
    claim.workloads = vec!["small".into()];
    claim.repetitions = 5;
    claim
}

fn row(variant: &str, workload: &str, repetition: u32, value: f64) -> ResultDocument {
    ResultDocument {
        schema_version: "1".into(),
        claim_id: "canvassync-redis-cold-start-latency".into(),
        project: "canvassync".into(),
        variant: variant.into(),
        workload: workload.into(),
        repetition,
        started_at: "2026-11-17T10:00:00Z".into(),
        duration_seconds: 0.01,
        metrics: vec![ResultMetric {
            name: "join_sync_latency_ms".into(),
            unit: "milliseconds".into(),
            value,
        }],
        environment: ResultEnvironment {
            commit: "abc123".into(),
            images: BTreeMap::from([("ws".into(), format!("sha256:{}", "a".repeat(64)))]),
            host_cpu: "Apple M5".into(),
            sut_cpuset: "0-1".into(),
            sut_memory_mb: 384,
            generator_cpu_peak_percent: 30.0,
            docker: "29.9.3".into(),
            os: "Darwin".into(),
        },
        limitations: vec![],
    }
}

fn pair(workload: &str, candidate: f64) -> Vec<ResultDocument> {
    (1..=5)
        .flat_map(|repetition| {
            [
                row("baseline", workload, repetition, 100.0),
                row("candidate", workload, repetition, candidate),
            ]
        })
        .collect()
}

#[test]
fn median_and_range_are_exact() {
    let summary = summarize(&[8.0, 2.0, 4.0, 6.0]).expect("summary");
    assert_eq!(
        (
            summary.median,
            summary.min,
            summary.max,
            summary.repetitions
        ),
        (5.0, 2.0, 8.0, 4)
    );
}

#[test]
fn verified_revised_and_not_supported_follow_target_and_direction() {
    assert_eq!(
        evaluate(&claim(), &pair("small", 40.0)).outcome,
        Outcome::Verified
    );
    assert_eq!(
        evaluate(&claim(), &pair("small", 70.0)).outcome,
        Outcome::Revised
    );
    assert_eq!(
        evaluate(&claim(), &pair("small", 100.0)).outcome,
        Outcome::NotSupported
    );
    assert_eq!(
        evaluate(&claim(), &pair("small", 120.0)).outcome,
        Outcome::NotSupported
    );
}

#[test]
fn conditional_names_only_supporting_workloads() {
    let mut claim = claim();
    claim.workloads.push("large".into());
    let mut rows = pair("small", 40.0);
    rows.extend(pair("large", 100.0));
    let verdict = evaluate(&claim, &rows);
    assert_eq!(verdict.outcome, Outcome::Conditional);
    assert!(verdict
        .suggested_wording
        .expect("wording")
        .contains("small"));
}

#[test]
fn wording_rounds_reduction_down() {
    let verdict = evaluate(&claim(), &pair("small", 40.1));
    assert_eq!(verdict.outcome, Outcome::Verified);
    assert!(verdict.suggested_wording.expect("wording").contains("59%"));
}

#[test]
fn missing_repetitions_cpu_environment_and_wide_range_are_inconclusive() {
    let rows = pair("small", 40.0);
    assert_eq!(
        evaluate(&claim(), &rows[..8]).outcome,
        Outcome::Inconclusive
    );
    let mut high_cpu = rows.clone();
    high_cpu[0].environment.generator_cpu_peak_percent = 71.0;
    assert!(evaluate(&claim(), &high_cpu).next_step.contains("70%"));
    let mut different = rows.clone();
    different[0].environment.host_cpu = "other".into();
    assert!(evaluate(&claim(), &different)
        .next_step
        .contains("Environments differ"));
    let mut wide = rows;
    wide[0].metrics[0].value = 10.0;
    wide[2].metrics[0].value = 200.0;
    assert!(evaluate(&claim(), &wide)
        .next_step
        .contains("range is wider"));
}

#[test]
fn behavior_pass_and_fail_include_observed_values() {
    let mut claim = claim();
    claim.kind = "behavior".into();
    claim.metric = "observed_pass".into();
    claim.behavior_expected_min = Some(1.0);
    let mut rows: Vec<_> = (1..=5)
        .map(|repetition| row("observed", "small", repetition, 1.0))
        .collect();
    for row in &mut rows {
        row.metrics[0].name = "observed_pass".into();
    }
    assert_eq!(evaluate(&claim, &rows).outcome, Outcome::Verified);
    rows[0].metrics[0].value = 0.0;
    let verdict = evaluate(&claim, &rows);
    assert_eq!(verdict.outcome, Outcome::NotSupported);
    assert_eq!(
        verdict.workloads[0]
            .candidate
            .as_ref()
            .expect("observed")
            .min,
        0.0
    );
}
