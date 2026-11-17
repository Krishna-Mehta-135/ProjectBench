use crate::{
    claims::Claim,
    results::{ResultDocument, ResultEnvironment},
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Verified,
    Revised,
    Conditional,
    NotSupported,
    Inconclusive,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub median: f64,
    pub min: f64,
    pub max: f64,
    pub repetitions: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadVerdict {
    pub workload: String,
    pub outcome: Outcome,
    pub baseline: Option<Summary>,
    pub candidate: Option<Summary>,
    pub relative_change_percent: Option<f64>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Verdict {
    pub claim_id: String,
    pub outcome: Outcome,
    pub next_step: String,
    pub suggested_wording: Option<String>,
    pub tolerance_percent: Option<f64>,
    pub workloads: Vec<WorkloadVerdict>,
}

pub fn summarize(values: &[f64]) -> Option<Summary> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    let median = if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    };
    Some(Summary {
        median,
        min: sorted[0],
        max: sorted[sorted.len() - 1],
        repetitions: sorted.len(),
    })
}

fn same_environment(left: &ResultEnvironment, right: &ResultEnvironment) -> bool {
    left.commit == right.commit
        && left.images == right.images
        && left.host_cpu == right.host_cpu
        && left.sut_cpuset == right.sut_cpuset
        && left.sut_memory_mb == right.sut_memory_mb
        && left.docker == right.docker
        && left.os == right.os
}

fn metric(result: &ResultDocument, name: &str) -> Option<f64> {
    result
        .metrics
        .iter()
        .find(|metric| metric.name == name)
        .map(|metric| metric.value)
}

fn inconclusive(workload: &str, reason: impl Into<String>) -> WorkloadVerdict {
    WorkloadVerdict {
        workload: workload.into(),
        outcome: Outcome::Inconclusive,
        baseline: None,
        candidate: None,
        relative_change_percent: None,
        reason: Some(reason.into()),
    }
}

fn repetition_count(rows: &[&ResultDocument]) -> usize {
    rows.iter()
        .map(|row| row.repetition)
        .collect::<HashSet<_>>()
        .len()
}

fn evaluate_comparative(
    claim: &Claim,
    workload: &str,
    rows: &[&ResultDocument],
) -> WorkloadVerdict {
    let baseline: Vec<_> = rows
        .iter()
        .copied()
        .filter(|row| row.variant == "baseline")
        .collect();
    let candidate: Vec<_> = rows
        .iter()
        .copied()
        .filter(|row| row.variant == "candidate")
        .collect();
    let required = claim.repetitions as usize;
    if repetition_count(&baseline) < required || repetition_count(&candidate) < required {
        return inconclusive(
            workload,
            format!("Need at least {required} repetitions per variant; rerun this workload."),
        );
    }
    let baseline_values: Vec<_> = baseline
        .iter()
        .filter_map(|row| metric(row, &claim.metric))
        .collect();
    let candidate_values: Vec<_> = candidate
        .iter()
        .filter_map(|row| metric(row, &claim.metric))
        .collect();
    if baseline_values.len() != baseline.len() || candidate_values.len() != candidate.len() {
        return inconclusive(
            workload,
            "Claim metric is missing; fix the harness and rerun.",
        );
    }
    let Some(base) = summarize(&baseline_values) else {
        return inconclusive(workload, "Baseline values are invalid; rerun.");
    };
    let Some(cand) = summarize(&candidate_values) else {
        return inconclusive(workload, "Candidate values are invalid; rerun.");
    };
    if base.median <= 0.0 {
        return inconclusive(
            workload,
            "Baseline median is zero; define a nonzero comparison and rerun.",
        );
    }
    let difference = match claim.direction.as_str() {
        "lower_is_better" => base.median - cand.median,
        "higher_is_better" => cand.median - base.median,
        _ => {
            return inconclusive(
                workload,
                "Claim direction is unsupported for comparative verdicts.",
            )
        }
    };
    let relative_change_percent = difference / base.median * 100.0;
    if base.max - base.min > difference.abs() || cand.max - cand.min > difference.abs() {
        return WorkloadVerdict {
            workload: workload.into(),
            outcome: Outcome::Inconclusive,
            baseline: Some(base),
            candidate: Some(cand),
            relative_change_percent: Some(relative_change_percent),
            reason: Some(
                "Observed range is wider than the median difference; rerun under steadier load."
                    .into(),
            ),
        };
    }
    let outcome = if difference <= 0.0 {
        Outcome::NotSupported
    } else if claim.target_percent.is_none_or(|target| {
        relative_change_percent >= target - claim.tolerance_percent.unwrap_or(0.0)
    }) {
        Outcome::Verified
    } else {
        Outcome::Revised
    };
    WorkloadVerdict {
        workload: workload.into(),
        outcome,
        baseline: Some(base),
        candidate: Some(cand),
        relative_change_percent: Some(relative_change_percent),
        reason: None,
    }
}

fn evaluate_behavior(claim: &Claim, workload: &str, rows: &[&ResultDocument]) -> WorkloadVerdict {
    let observed: Vec<_> = rows
        .iter()
        .copied()
        .filter(|row| row.variant == "candidate" || row.variant == "observed")
        .collect();
    if repetition_count(&observed) < claim.repetitions as usize {
        return inconclusive(
            workload,
            format!(
                "Need at least {} observed repetitions; rerun this workload.",
                claim.repetitions
            ),
        );
    }
    let values: Vec<_> = observed
        .iter()
        .filter_map(|row| metric(row, &claim.metric))
        .collect();
    if values.len() != observed.len() {
        return inconclusive(
            workload,
            "Observed metric is missing; fix the harness and rerun.",
        );
    }
    let Some(summary) = summarize(&values) else {
        return inconclusive(workload, "Observed values are invalid; rerun.");
    };
    let outcome = if values
        .iter()
        .all(|value| *value >= claim.behavior_expected_min.unwrap_or(1.0))
    {
        Outcome::Verified
    } else {
        Outcome::NotSupported
    };
    WorkloadVerdict {
        workload: workload.into(),
        outcome,
        baseline: None,
        candidate: Some(summary),
        relative_change_percent: None,
        reason: None,
    }
}

fn overall_outcome(workloads: &[WorkloadVerdict]) -> Outcome {
    if workloads
        .iter()
        .any(|workload| workload.outcome == Outcome::Inconclusive)
    {
        Outcome::Inconclusive
    } else if workloads
        .iter()
        .all(|workload| workload.outcome == Outcome::Verified)
    {
        Outcome::Verified
    } else if workloads
        .iter()
        .all(|workload| workload.outcome == Outcome::NotSupported)
    {
        Outcome::NotSupported
    } else if workloads
        .iter()
        .all(|workload| workload.outcome == Outcome::Revised)
    {
        Outcome::Revised
    } else {
        Outcome::Conditional
    }
}

fn next_step(outcome: Outcome, workloads: &[WorkloadVerdict]) -> String {
    match outcome {
        Outcome::Verified => "Publish. Keep this result set as the baseline for reruns.".into(),
        Outcome::Revised => {
            "Review the suggested wording with the measured value and workload.".into()
        }
        Outcome::Conditional => {
            "Use wording that names only the workloads where the claim holds.".into()
        }
        Outcome::NotSupported => {
            "Remove the number and keep the mechanism, or change the code and rerun.".into()
        }
        Outcome::Inconclusive => workloads
            .iter()
            .filter_map(|workload| {
                workload
                    .reason
                    .as_ref()
                    .map(|reason| format!("{}: {reason}", workload.workload))
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn suggested_wording(
    claim: &Claim,
    outcome: Outcome,
    workloads: &[WorkloadVerdict],
) -> Option<String> {
    if matches!(outcome, Outcome::Inconclusive | Outcome::NotSupported) {
        return None;
    }
    let best = workloads
        .iter()
        .filter(|workload| matches!(workload.outcome, Outcome::Verified | Outcome::Revised))
        .filter_map(|workload| {
            workload
                .relative_change_percent
                .map(|change| (workload, change))
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))?;
    let metric_name = match claim.metric.as_str() {
        "join_sync_latency_ms" => "median cold-join latency",
        "outbound_frames_per_receiver" => "outbound WebSocket frames per receiver",
        _ => claim.metric.as_str(),
    };
    Some(format!(
        "Reduced {metric_name} by {}% for {} in the measured local workload.",
        best.1.floor() as i64,
        best.0.workload.replace(['_', '-'], " ")
    ))
}

pub fn evaluate(claim: &Claim, results: &[ResultDocument]) -> Verdict {
    let mut workloads = Vec::new();
    for workload in &claim.workloads {
        let rows: Vec<_> = results
            .iter()
            .filter(|result| result.workload == *workload)
            .collect();
        if rows.is_empty() {
            workloads.push(inconclusive(
                workload,
                "No results for this listed workload; run and ingest it.",
            ));
            continue;
        }
        if rows
            .iter()
            .any(|row| row.environment.generator_cpu_peak_percent > 70.0)
        {
            workloads.push(inconclusive(
                workload,
                "Generator CPU exceeded 70%; rerun with a lighter generator.",
            ));
            continue;
        }
        if rows
            .iter()
            .any(|row| !same_environment(&rows[0].environment, &row.environment))
        {
            workloads.push(inconclusive(
                workload,
                "Environments differ between variants or repetitions; rerun under one setup.",
            ));
            continue;
        }
        workloads.push(match claim.kind.as_str() {
            "comparative" => evaluate_comparative(claim, workload, &rows),
            "behavior" => evaluate_behavior(claim, workload, &rows),
            _ => inconclusive(workload, "This claim kind has no result verdict rule."),
        });
    }
    let outcome = overall_outcome(&workloads);
    Verdict {
        claim_id: claim.id.clone(),
        outcome,
        next_step: next_step(outcome, &workloads),
        suggested_wording: suggested_wording(claim, outcome, &workloads),
        tolerance_percent: claim.tolerance_percent,
        workloads,
    }
}

#[cfg(test)]
#[path = "verdict_tests.rs"]
mod tests;
