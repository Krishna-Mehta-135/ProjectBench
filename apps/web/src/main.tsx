import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { api, ProjectSummary, RunDetail, RunSummary } from "./api";
import "./style.css";

function App() {
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [selectedRun, setSelectedRun] = useState<RunDetail | null>(null);
  const [runLoading, setRunLoading] = useState(false);
  const [runError, setRunError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.projects(), api.runs()]).then(([projectData, runData]) => { setProjects(projectData); setRuns(runData); setSelectedRunId(runData[0]?.id ?? null); }).catch((cause: Error) => setError(cause.message)).finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (!selectedRunId) return;
    setRunLoading(true);
    setRunError(null);
    api.run(selectedRunId).then(setSelectedRun).catch((cause: Error) => setRunError(cause.message)).finally(() => setRunLoading(false));
  }, [selectedRunId]);

  return <main className="shell">
    <header><div><p className="eyebrow">METRICS / EVIDENCE</p><h1>Performance, with provenance.</h1><p className="lede">A private control plane for reproducible synthetic performance evidence.</p></div><button disabled>New run</button></header>
    <section className="notice"><strong>Synthetic traffic only.</strong> Results from k6 virtual users are not measurements of real users or browser rendering.</section>
    {loading && <section className="panel"><p>Loading projects…</p></section>}
    {error && <section className="panel error"><h2>API unavailable</h2><p>{error}</p><p>Start the Rust API on <code>127.0.0.1:8080</code> to load project data.</p></section>}
    {!loading && !error && projects.length === 0 && <section className="panel"><div><p className="eyebrow">PROJECTS</p><h2>No projects configured</h2><p>Configure a project and environment before creating a run. Missing data is not treated as zero.</p></div></section>}
    {!loading && !error && projects.length > 0 && <section className="grid">{projects.map((project) => <article key={project.id}><div className="card-head"><span className="dot"/><span>{project.environment}</span></div><h2>{project.name}</h2><p>{project.latest_run_id ? "Baseline available" : "No baseline yet"}</p><a href="#scenarios">View scenarios →</a></article>)}</section>}
    {!loading && !error && runs.length > 0 && <section className="history-layout">
      <div className="panel history-list"><div><p className="eyebrow">RUN HISTORY</p><h2>{runs.length} recorded run{runs.length === 1 ? "" : "s"}</h2><p>Every run is synthetic k6 evidence with a retained status and report reference.</p></div><div className="run-list">{runs.slice(0, 10).map((run) => <button className={`run-row ${selectedRunId === run.id ? "selected" : ""}`} key={run.id} onClick={() => setSelectedRunId(run.id)}><span><strong>{run.status}</strong><small>{run.id.slice(0, 8)} · {run.synthetic_traffic ? "synthetic" : "unknown"}</small></span><span className={run.report_available ? "ready" : "muted"}>{run.report_available ? "Report ready" : "No report"}</span></button>)}</div></div>
      <div className="panel report-panel"><p className="eyebrow">REPORT DETAIL</p>{runLoading && <p>Loading report…</p>}{runError && <div className="error-copy"><h2>Report unavailable</h2><p>{runError}</p></div>}{!runLoading && !runError && selectedRun && <ReportDetail run={selectedRun} />}</div>
    </section>}
  </main>;
}

function ReportDetail({ run }: { run: RunDetail }) {
  const report = run.report;
  return <>
    <div className="report-title"><div><h2>{report?.scenario.name ?? "Run report"}</h2><p>{run.id} · {run.target_origin}</p></div><span className={`status status-${run.status}`}>{run.status}</span></div>
    {!report && <><p>This run has no persisted report yet. It may still be queued or running, or it may have failed before report creation.</p>{run.failure_reason && <p className="failure-reason"><strong>Failure reason:</strong> {run.failure_reason}</p>}</>}
    {report && <>
      <div className="metric-grid">{[["Requests", report.metrics.request_count.toLocaleString()], ["Throughput", `${report.metrics.throughput_per_second.toFixed(2)}/s`], ["Error rate", `${(report.metrics.error_rate * 100).toFixed(2)}%`], ["p95", report.metrics.p95_ms === null ? "—" : `${report.metrics.p95_ms.toFixed(1)} ms`]].map(([label, value]) => <div className="metric" key={label}><small>{label}</small><strong>{value}</strong></div>)}</div>
      <div className="thresholds"><h3>Thresholds</h3>{report.thresholds.map((threshold) => <p key={threshold.name}><span className={threshold.passed ? "pass" : "fail"}>{threshold.passed ? "PASS" : "FAIL"}</span> {threshold.name}</p>)}</div>
      <div className="report-meta"><p><strong>Synthetic k6 traffic.</strong> This is controlled virtual-user evidence, not real-user or browser-rendering data.</p><p>Scenario {report.scenario.version} · {report.workload.vus} VUs · {report.workload.duration_seconds}s</p>{run.failure_reason && <p className="failure-reason"><strong>Failure reason:</strong> {run.failure_reason}</p>}<p>Created {new Date(run.created_at).toLocaleString()} · {run.finished_at ? `Finished ${new Date(run.finished_at).toLocaleString()}` : "Still active"}</p></div>
      <div className="artifact-links"><h3>Artifacts</h3>{report.artifacts.map((artifact) => <a key={artifact.kind} href={api.artifactUrl(run.id, artifact.kind)} target="_blank" rel="noreferrer">Download {artifact.kind} →</a>)}</div>
    </>}
  </>;
}

createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);

