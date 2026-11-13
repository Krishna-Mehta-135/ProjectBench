import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { api, ProjectSummary, RunSummary } from "./api";
import "./style.css";

function App() {
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.projects(), api.runs()]).then(([projectData, runData]) => { setProjects(projectData); setRuns(runData); }).catch((cause: Error) => setError(cause.message)).finally(() => setLoading(false));
  }, []);

  return <main className="shell">
    <header><div><p className="eyebrow">METRICS / EVIDENCE</p><h1>Performance, with provenance.</h1><p className="lede">A private control plane for reproducible synthetic performance evidence.</p></div><button disabled>New run</button></header>
    <section className="notice"><strong>Synthetic traffic only.</strong> Results from k6 virtual users are not measurements of real users or browser rendering.</section>
    {loading && <section className="panel"><p>Loading projects…</p></section>}
    {error && <section className="panel error"><h2>API unavailable</h2><p>{error}</p><p>Start the Rust API on <code>127.0.0.1:8080</code> to load project data.</p></section>}
    {!loading && !error && projects.length === 0 && <section className="panel"><div><p className="eyebrow">PROJECTS</p><h2>No projects configured</h2><p>Configure a project and environment before creating a run. Missing data is not treated as zero.</p></div></section>}
    {!loading && !error && projects.length > 0 && <section className="grid">{projects.map((project) => <article key={project.id}><div className="card-head"><span className="dot"/><span>{project.environment}</span></div><h2>{project.name}</h2><p>{project.latest_run_id ? "Baseline available" : "No baseline yet"}</p><a href="#scenarios">View scenarios →</a></article>)}</section>}
    {!loading && !error && runs.length > 0 && <section className="panel"><div><p className="eyebrow">RUN HISTORY</p><h2>{runs.length} recorded run{runs.length === 1 ? "" : "s"}</h2><p>Every run is synthetic k6 evidence with a retained status and report reference.</p></div><div>{runs.slice(0, 5).map((run) => <p key={run.id}><strong>{run.status}</strong> · {run.synthetic_traffic ? "synthetic" : "unknown"} · {run.report_available ? "report ready" : "report unavailable"}</p>)}</div></section>}
  </main>;
}

createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);

