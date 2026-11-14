import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { api, Environment, ProjectSummary, RunDetail, RunSummary, Scenario } from "./api";
import { canCancelRun, formatRunStatus } from "./runPresentation";
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
  const [canceling, setCanceling] = useState(false);
  const [showNewRun, setShowNewRun] = useState(false);
  const [environments, setEnvironments] = useState<Environment[]>([]);
  const [scenarios, setScenarios] = useState<Scenario[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState("");
  const [selectedEnvironmentId, setSelectedEnvironmentId] = useState("");
  const [selectedScenarioId, setSelectedScenarioId] = useState("");
  const [vus, setVus] = useState(1);
  const [duration, setDuration] = useState(60);
  const [thinkTime, setThinkTime] = useState(2);
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    Promise.all([api.projects(), api.runs()]).then(([projectData, runData]) => { setProjects(projectData); setRuns(runData); setSelectedRunId(runData[0]?.id ?? null); }).catch((cause: Error) => setError(cause.message)).finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (!selectedProjectId) return;
    Promise.all([api.environments(selectedProjectId), api.scenarios(selectedProjectId)]).then(([envs, scenarioData]) => {
      setEnvironments(envs.filter((environment) => environment.enabled));
      setScenarios(scenarioData);
      setSelectedEnvironmentId(envs.find((environment) => environment.enabled)?.id ?? "");
      setSelectedScenarioId(scenarioData[0]?.id ?? "");
    }).catch((cause: Error) => setRunError(cause.message));
  }, [selectedProjectId]);

  async function createAndExecuteRun(event: React.FormEvent) {
    event.preventDefault();
    const environment = environments.find((item) => item.id === selectedEnvironmentId);
    const scenario = scenarios.find((item) => item.id === selectedScenarioId);
    if (!selectedProjectId || !environment || !scenario) return;
    setCreating(true);
    setRunError(null);
    try {
      const created = await api.createRun({ project_id: selectedProjectId, environment_id: environment.id, scenario_id: scenario.id, target_origin: environment.base_url, scenario: { name: scenario.name, version: scenario.version, script_path: scenario.script_path, routes: scenario.config_snapshot.routes ?? [] }, workload: { vus, duration_seconds: duration, think_time_seconds: thinkTime }, confirmed: true });
      await api.executeRun(created.id);
      setShowNewRun(false);
      const updatedRuns = await api.runs();
      setRuns(updatedRuns);
      setSelectedRunId(created.id);
    } catch (cause) {
      setRunError(cause instanceof Error ? cause.message : "Run creation failed");
    } finally { setCreating(false); }
  }

  useEffect(() => {
    if (!selectedRunId) return;
    setRunLoading(true);
    setRunError(null);
    api.run(selectedRunId).then(setSelectedRun).catch((cause: Error) => setRunError(cause.message)).finally(() => setRunLoading(false));
  }, [selectedRunId]);

  async function cancelSelectedRun() {
    if (!selectedRun || !canCancelRun(selectedRun.status) || !window.confirm("Cancel this run? The process will stop and any partial artifacts will be retained.")) return;
    setCanceling(true);
    setRunError(null);
    try {
      await api.cancelRun(selectedRun.id);
      const [updatedRuns, updatedRun] = await Promise.all([api.runs(), api.run(selectedRun.id)]);
      setRuns(updatedRuns);
      setSelectedRun(updatedRun);
    } catch (cause) {
      setRunError(cause instanceof Error ? cause.message : "Cancellation failed");
    } finally {
      setCanceling(false);
    }
  }

  return <main className="shell">
    <header><div><p className="eyebrow">METRICS / EVIDENCE</p><h1>Performance, with provenance.</h1><p className="lede">A private control plane for reproducible synthetic performance evidence.</p></div><button onClick={() => { setSelectedProjectId(projects[0]?.id ?? ""); setShowNewRun(true); }} disabled={loading || projects.length === 0}>New run</button></header>
    <section className="notice"><strong>Synthetic traffic only.</strong> Results from k6 virtual users are not measurements of real users or browser rendering.</section>
    {loading && <section className="panel"><p>Loading projects…</p></section>}
    {error && <section className="panel error"><h2>API unavailable</h2><p>{error}</p><p>Start the Rust API on <code>127.0.0.1:8080</code> to load project data.</p></section>}
    {!loading && !error && projects.length === 0 && <section className="panel"><div><p className="eyebrow">PROJECTS</p><h2>No projects configured</h2><p>Configure a project and environment before creating a run. Missing data is not treated as zero.</p></div></section>}
    {!loading && !error && projects.length > 0 && <section className="grid">{projects.map((project) => <article key={project.id}><div className="card-head"><span className="dot"/><span>{project.environment}</span></div><h2>{project.name}</h2><p>{project.latest_run_id ? "Baseline available" : "No baseline yet"}</p><a href="#scenarios">View scenarios →</a></article>)}</section>}
    {showNewRun && <section className="panel"><div><p className="eyebrow">NEW RUN</p><h2>Review effective workload</h2><form className="run-form" onSubmit={(event) => void createAndExecuteRun(event)}><label>Project<select value={selectedProjectId} onChange={(event) => setSelectedProjectId(event.target.value)}>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label><label>Environment<select value={selectedEnvironmentId} onChange={(event) => setSelectedEnvironmentId(event.target.value)}>{environments.map((environment) => <option key={environment.id} value={environment.id}>{environment.name} · {environment.base_url}</option>)}</select></label><label>Scenario<select value={selectedScenarioId} onChange={(event) => setSelectedScenarioId(event.target.value)}>{scenarios.map((scenario) => <option key={scenario.id} value={scenario.id}>{scenario.name} v{scenario.version}</option>)}</select></label><div className="form-grid"><label>VUs<input type="number" min="1" value={vus} onChange={(event) => setVus(Number(event.target.value))}/></label><label>Duration (s)<input type="number" min="1" value={duration} onChange={(event) => setDuration(Number(event.target.value))}/></label><label>Think time (s)<input type="number" min="0" value={thinkTime} onChange={(event) => setThinkTime(Number(event.target.value))}/></label></div><p className="muted">The server revalidates the target, scenario identity, routes, and workload caps before queueing.</p><div><button type="submit" disabled={creating || !selectedEnvironmentId || !selectedScenarioId}>{creating ? "Starting…" : "Confirm and start"}</button> <button type="button" onClick={() => setShowNewRun(false)}>Cancel</button></div></form></div></section>}
    {!loading && !error && runs.length > 0 && <section className="history-layout">
      <div className="panel history-list"><div><p className="eyebrow">RUN HISTORY</p><h2>{runs.length} recorded run{runs.length === 1 ? "" : "s"}</h2><p>Every run is synthetic k6 evidence with a retained status and report reference.</p></div><div className="run-list">{runs.slice(0, 10).map((run) => <button className={`run-row ${selectedRunId === run.id ? "selected" : ""}`} key={run.id} onClick={() => setSelectedRunId(run.id)}><span><strong>{run.status}</strong><small>{run.id.slice(0, 8)} · {run.synthetic_traffic ? "synthetic" : "unknown"}</small></span><span className={run.report_available ? "ready" : "muted"}>{run.report_available ? "Report ready" : "No report"}</span></button>)}</div></div>
      <div className="panel report-panel"><p className="eyebrow">REPORT DETAIL</p>{runLoading && <p>Loading report…</p>}{runError && <div className="error-copy"><h2>Run action unavailable</h2><p>{runError}</p></div>}{!runLoading && !runError && selectedRun && <ReportDetail run={selectedRun} canceling={canceling} onCancel={cancelSelectedRun} />}</div>
    </section>}
  </main>;
}

function ReportDetail({ run, canceling, onCancel }: { run: RunDetail; canceling: boolean; onCancel: () => Promise<void> }) {
  const report = run.report;
  return <>
    <div className="report-title"><div><h2>{report?.scenario.name ?? "Run report"}</h2><p>{run.id} · {run.target_origin}</p></div><div className="status-actions"><span className={`status status-${run.status}`}>{formatRunStatus(run.status)}</span>{canCancelRun(run.status) && <button className="cancel-button" onClick={() => void onCancel()} disabled={canceling}>{canceling ? "Canceling…" : "Cancel run"}</button>}</div></div>
    <div className="lifecycle-meta"><span>Created {new Date(run.created_at).toLocaleString()}</span>{run.started_at && <span>Started {new Date(run.started_at).toLocaleString()}</span>}{run.finished_at && <span>Finished {new Date(run.finished_at).toLocaleString()}</span>}</div>
    {!report && <><p>This run has no persisted report yet. It may still be queued or running, or it may have failed or been canceled before report creation.</p>{run.failure_reason && <p className="failure-reason"><strong>Reason:</strong> {run.failure_reason}</p>}</>}
    {report && <>
      <div className="metric-grid">{[["Requests", report.metrics.request_count.toLocaleString()], ["Throughput", `${report.metrics.throughput_per_second.toFixed(2)}/s`], ["Error rate", `${(report.metrics.error_rate * 100).toFixed(2)}%`], ["p95", report.metrics.p95_ms === null ? "—" : `${report.metrics.p95_ms.toFixed(1)} ms`]].map(([label, value]) => <div className="metric" key={label}><small>{label}</small><strong>{value}</strong></div>)}</div>
      <div className="thresholds"><h3>Thresholds</h3>{report.thresholds.map((threshold) => <p key={threshold.name}><span className={threshold.passed ? "pass" : "fail"}>{threshold.passed ? "PASS" : "FAIL"}</span> {threshold.name}</p>)}</div>
      <div className="report-meta"><p><strong>Synthetic k6 traffic.</strong> This is controlled virtual-user evidence, not real-user or browser-rendering data.</p><p>Scenario {report.scenario.version} · {report.workload.vus} VUs · {report.workload.duration_seconds}s</p>{run.failure_reason && <p className="failure-reason"><strong>Reason:</strong> {run.failure_reason}</p>}</div>
      <div className="artifact-links"><h3>Artifacts</h3>{report.artifacts.map((artifact) => <a key={artifact.kind} href={api.artifactUrl(run.id, artifact.kind)} target="_blank" rel="noreferrer">Download {artifact.kind} →</a>)}</div>
    </>}
  </>;
}

createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);

