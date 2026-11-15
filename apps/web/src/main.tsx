import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  api,
  Environment,
  ProjectSummary,
  RunDetail,
  RunSummary,
  Scenario,
  TimeBucket,
  BaselineSummary,
  PreflightResponse,
  setOperatorToken,
} from "./api";
import { canCancelRun, formatRunStatus } from "./runPresentation";
import "./style.css";

function App() {
  const [view, setView] = useState<"portfolio" | "lab">("portfolio");
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
  const [preflight, setPreflight] = useState<PreflightResponse | null>(null);
  const [baselines, setBaselines] = useState<
    Record<string, BaselineSummary | null>
  >({});
  const [showRegistry, setShowRegistry] = useState(false);
  const [tokenDraft, setTokenDraft] = useState("");
  const [needsToken, setNeedsToken] = useState(false);

  useEffect(() => {
    Promise.all([api.projects(), api.runs()])
      .then(async ([projectData, runData]) => {
        setProjects(projectData);
        setRuns(runData);
        setSelectedRunId(runData[0]?.id ?? null);
        const baselineEntries = await Promise.all(
          projectData.map(
            async (project) =>
              [
                project.id,
                await api.baseline(project.id).catch(() => null),
              ] as const,
          ),
        );
        setBaselines(Object.fromEntries(baselineEntries));
        setNeedsToken(false);
      })
      .catch((cause: Error) => {
        setError(cause.message);
        setNeedsToken(cause.message.includes("401"));
      })
      .finally(() => setLoading(false));
  }, []);

  function applyOperatorToken(event: React.FormEvent) {
    event.preventDefault();
    setOperatorToken(tokenDraft);
    setError(null);
    setLoading(true);
    setNeedsToken(false);
    Promise.all([api.projects(), api.runs()])
      .then(([projectData, runData]) => {
        setProjects(projectData);
        setRuns(runData);
        setSelectedRunId(runData[0]?.id ?? null);
      })
      .catch((cause: Error) => {
        setError(cause.message);
        setNeedsToken(true);
      })
      .finally(() => setLoading(false));
  }

  useEffect(() => {
    if (!selectedProjectId) return;
    Promise.all([
      api.environments(selectedProjectId),
      api.scenarios(selectedProjectId),
    ])
      .then(([envs, scenarioData]) => {
        setEnvironments(envs.filter((environment) => environment.enabled));
        setScenarios(scenarioData);
        setSelectedEnvironmentId(
          envs.find((environment) => environment.enabled)?.id ?? "",
        );
        setSelectedScenarioId(scenarioData[0]?.id ?? "");
      })
      .catch((cause: Error) => setRunError(cause.message));
  }, [selectedProjectId]);

  async function createAndExecuteRun(event: React.FormEvent) {
    event.preventDefault();
    const environment = environments.find(
      (item) => item.id === selectedEnvironmentId,
    );
    const scenario = scenarios.find((item) => item.id === selectedScenarioId);
    if (!selectedProjectId || !environment || !scenario) return;
    setCreating(true);
    setRunError(null);
    try {
      const snapshot = {
        name: scenario.name,
        version: scenario.version,
        script_path: scenario.script_path,
        routes: scenario.config_snapshot.routes ?? [],
        secret_refs: scenario.secret_refs,
      };
      const review = await api.preflight({
        target_origin: environment.base_url,
        scenario: snapshot,
        workload: {
          vus,
          duration_seconds: duration,
          think_time_seconds: thinkTime,
        },
      });
      setPreflight(review);
      if (
        !window.confirm(
          `Preflight approved for ${review.target_origin}. ${review.warnings.length ? `Warnings: ${review.warnings.join("; ")}` : "No warnings."} Start this synthetic run?`,
        )
      )
        return;
      const created = await api.createRun({
        project_id: selectedProjectId,
        environment_id: environment.id,
        scenario_id: scenario.id,
        target_origin: environment.base_url,
        scenario: snapshot,
        workload: {
          vus,
          duration_seconds: duration,
          think_time_seconds: thinkTime,
        },
        confirmed: true,
      });
      await api.executeRun(created.id);
      setShowNewRun(false);
      const updatedRuns = await api.runs();
      setRuns(updatedRuns);
      setSelectedRunId(created.id);
    } catch (cause) {
      setRunError(
        cause instanceof Error ? cause.message : "Run creation failed",
      );
    } finally {
      setCreating(false);
    }
  }

  useEffect(() => {
    if (!selectedRunId) return;
    setRunLoading(true);
    setRunError(null);
    api
      .run(selectedRunId)
      .then(setSelectedRun)
      .catch((cause: Error) => setRunError(cause.message))
      .finally(() => setRunLoading(false));
  }, [selectedRunId]);

  async function cancelSelectedRun() {
    if (
      !selectedRun ||
      !canCancelRun(selectedRun.status) ||
      !window.confirm(
        "Cancel this run? The process will stop and any partial artifacts will be retained.",
      )
    )
      return;
    setCanceling(true);
    setRunError(null);
    try {
      await api.cancelRun(selectedRun.id);
      const [updatedRuns, updatedRun] = await Promise.all([
        api.runs(),
        api.run(selectedRun.id),
      ]);
      setRuns(updatedRuns);
      setSelectedRun(updatedRun);
    } catch (cause) {
      setRunError(
        cause instanceof Error ? cause.message : "Cancellation failed",
      );
    } finally {
      setCanceling(false);
    }
  }

  return (
    <main className="shell">
      <nav className="topbar" aria-label="Primary navigation">
        <button
          className="wordmark"
          onClick={() => setView("portfolio")}
          aria-label="ProjectBench home"
        >
          <span className="mark">M</span>METRICS<span className="slash">/</span>
          KRISHNA
        </button>
        <div className="nav-links">
          <a href="#projects">Projects</a>
          <a href="#method">Method</a>
          <button
            className="nav-cta"
            onClick={() => setView(view === "portfolio" ? "lab" : "portfolio")}
          >
            {view === "portfolio" ? "Open lab ↗" : "Portfolio"}
          </button>
        </div>
      </nav>
      {view === "portfolio" ? (
        <Portfolio
          projects={projects}
          loading={loading}
          openLab={() => setView("lab")}
        />
      ) : (
        <>
          <header className="lab-header">
            <div>
              <p className="eyebrow">PRIVATE CONTROL ROOM</p>
              <h1>Evidence, under load.</h1>
              <p className="lede">
                Configure controlled synthetic runs, preserve their context, and
                compare what changed.
              </p>
            </div>
            <div className="header-actions">
              <button
                className="secondary"
                onClick={() => setShowRegistry((value) => !value)}
              >
                {showRegistry ? "Hide setup" : "Configure targets"}
              </button>
              <button
                onClick={() => {
                  setSelectedProjectId(projects[0]?.id ?? "");
                  setShowNewRun(true);
                  setPreflight(null);
                }}
                disabled={loading || projects.length === 0}
              >
                New run
              </button>
            </div>
          </header>
          <section className="notice">
            <strong>Synthetic traffic only.</strong> Results from k6 virtual
            users are not measurements of real users or browser rendering.
          </section>
          {(needsToken || tokenDraft) && (
            <section className="panel access-panel">
              <p className="eyebrow">OPERATOR ACCESS</p>
              <h2>Connect to the private ProjectBench API</h2>
              <p>
                Enter the operator token for this browser session. It stays in
                memory and is never bundled into the frontend.
              </p>
              <form onSubmit={applyOperatorToken}>
                <input
                  type="password"
                  value={tokenDraft}
                  onChange={(event) => setTokenDraft(event.target.value)}
                  placeholder="METRICS_OPERATOR_TOKEN"
                  autoComplete="off"
                />
                <button type="submit">Connect</button>
              </form>
            </section>
          )}
          {loading && (
            <section className="panel">
              <p>Loading projects…</p>
            </section>
          )}
          {error && (
            <section className="panel error">
              <h2>API unavailable</h2>
              <p>{error}</p>
              <p>
                Start the Rust API on <code>127.0.0.1:8080</code> to load
                project data.
              </p>
            </section>
          )}
          {!loading && !error && projects.length === 0 && (
            <section className="panel">
              <div>
                <p className="eyebrow">PROJECTS</p>
                <h2>No projects configured</h2>
                <p>
                  Configure a project and environment before creating a run.
                  Missing data is not treated as zero.
                </p>
              </div>
            </section>
          )}
          {!loading && !error && projects.length > 0 && (
            <section className="grid">
              {projects.map((project) => (
                <article key={project.id}>
                  <div className="card-head">
                    <span className="dot" />
                    <span>{project.environment}</span>
                  </div>
                  <h2>{project.name}</h2>
                  <p>
                    {baselines[project.id]
                      ? `Selected baseline ${baselines[project.id]!.run_id.slice(0, 8)}`
                      : "No baseline selected"}
                  </p>
                  <a href="#scenarios">View scenarios →</a>
                </article>
              ))}
            </section>
          )}
          {showRegistry && (
            <RegistryPanel
              projects={projects}
              onChanged={() => window.location.reload()}
            />
          )}
          {showNewRun && (
            <section className="panel">
              <div>
                <p className="eyebrow">NEW RUN</p>
                <h2>Review effective workload</h2>
                <form
                  className="run-form"
                  onSubmit={(event) => void createAndExecuteRun(event)}
                >
                  <label>
                    Project
                    <select
                      value={selectedProjectId}
                      onChange={(event) => {
                        setSelectedProjectId(event.target.value);
                        setPreflight(null);
                      }}
                    >
                      {projects.map((project) => (
                        <option key={project.id} value={project.id}>
                          {project.name}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    Environment
                    <select
                      value={selectedEnvironmentId}
                      onChange={(event) => {
                        setSelectedEnvironmentId(event.target.value);
                        setPreflight(null);
                      }}
                    >
                      {environments.map((environment) => (
                        <option key={environment.id} value={environment.id}>
                          {environment.name} · {environment.base_url}
                        </option>
                      ))}
                    </select>
                  </label>
                  {environments.length === 0 && (
                    <div className="preflight safety-gate">
                      <strong>Run blocked by safety gate</strong>
                      <p>
                        This target has no enabled environment. Review the
                        origin, account, seeded data, workload, hard stop, and
                        rollback criteria before enabling it.
                      </p>
                    </div>
                  )}
                  <label>
                    Scenario
                    <select
                      value={selectedScenarioId}
                      onChange={(event) => {
                        setSelectedScenarioId(event.target.value);
                        setPreflight(null);
                      }}
                    >
                      {scenarios.map((scenario) => (
                        <option key={scenario.id} value={scenario.id}>
                          {scenario.name} v{scenario.version}
                        </option>
                      ))}
                    </select>
                  </label>
                  <div className="form-grid">
                    <label>
                      VUs
                      <input
                        type="number"
                        min="1"
                        value={vus}
                        onChange={(event) => {
                          setVus(Number(event.target.value));
                          setPreflight(null);
                        }}
                      />
                    </label>
                    <label>
                      Duration (s)
                      <input
                        type="number"
                        min="1"
                        value={duration}
                        onChange={(event) => {
                          setDuration(Number(event.target.value));
                          setPreflight(null);
                        }}
                      />
                    </label>
                    <label>
                      Think time (s)
                      <input
                        type="number"
                        min="0"
                        value={thinkTime}
                        onChange={(event) => {
                          setThinkTime(Number(event.target.value));
                          setPreflight(null);
                        }}
                      />
                    </label>
                  </div>
                  <p className="muted">
                    The server revalidates the target, scenario identity,
                    routes, and workload caps before queueing.
                  </p>
                  {preflight && (
                    <div className="preflight">
                      <strong>Preflight approved</strong>
                      <p>
                        Target: <code>{preflight.target_origin}</code>
                      </p>
                      <p>
                        Scenario hash: <code>{preflight.scenario_hash}</code>
                      </p>
                      <p>
                        Hard stop: {preflight.workload.duration_seconds}s ·{" "}
                        {preflight.workload.vus} VUs
                      </p>
                      {preflight.warnings.length > 0 && (
                        <ul>
                          {preflight.warnings.map((warning) => (
                            <li key={warning}>{warning}</li>
                          ))}
                        </ul>
                      )}
                    </div>
                  )}
                  <div>
                    <button
                      type="submit"
                      disabled={
                        creating ||
                        !selectedEnvironmentId ||
                        !selectedScenarioId
                      }
                    >
                      {creating ? "Starting…" : "Review preflight and start"}
                    </button>{" "}
                    <button type="button" onClick={() => setShowNewRun(false)}>
                      Cancel
                    </button>
                  </div>
                </form>
              </div>
            </section>
          )}
          {!loading && !error && runs.length > 0 && (
            <section className="history-layout">
              <div className="panel history-list">
                <div>
                  <p className="eyebrow">RUN HISTORY</p>
                  <h2>
                    {runs.length} recorded run{runs.length === 1 ? "" : "s"}
                  </h2>
                  <p>
                    Every run is synthetic k6 evidence with a retained status
                    and report reference.
                  </p>
                </div>
                <div className="run-list">
                  {runs.slice(0, 10).map((run) => (
                    <button
                      className={`run-row ${selectedRunId === run.id ? "selected" : ""}`}
                      key={run.id}
                      onClick={() => setSelectedRunId(run.id)}
                    >
                      <span>
                        <strong>{run.status}</strong>
                        <small>
                          {run.id.slice(0, 8)} ·{" "}
                          {run.synthetic_traffic ? "synthetic" : "unknown"}
                        </small>
                      </span>
                      <span
                        className={run.report_available ? "ready" : "muted"}
                      >
                        {run.report_available ? "Report ready" : "No report"}
                      </span>
                    </button>
                  ))}
                </div>
              </div>
              <div className="panel report-panel">
                <p className="eyebrow">REPORT DETAIL</p>
                {runLoading && <p>Loading report…</p>}
                {runError && (
                  <div className="error-copy">
                    <h2>Run action unavailable</h2>
                    <p>{runError}</p>
                  </div>
                )}
                {!runLoading && !runError && selectedRun && (
                  <ReportDetail
                    run={selectedRun}
                    canceling={canceling}
                    onCancel={cancelSelectedRun}
                  />
                )}
              </div>
            </section>
          )}
          {!loading && !error && <ComparisonPanel runs={runs} />}
        </>
      )}
    </main>
  );
}

function Portfolio({
  projects,
  loading,
  openLab,
}: {
  projects: ProjectSummary[];
  loading: boolean;
  openLab: () => void;
}) {
  const names = projects.map((project) => project.name);
  const hasCanvassync = names.includes("Canvassync");
  const hasKnowdex = names.includes("Knowdex");
  return (
    <>
      <section className="portfolio-hero">
        <div className="hero-copy">
          <p className="eyebrow">INDEPENDENT BUILDER · INDIA</p>
          <h1>
            I make thoughtful products
            <br />
            <em>and measure them.</em>
          </h1>
          <p className="hero-lede">
            A portfolio of systems built with care—from collaborative creative
            tools to AI knowledge work—backed by repeatable performance
            evidence.
          </p>
          <div className="hero-actions">
            <a className="button-link" href="#projects">
              Explore the work <span>↓</span>
            </a>
            <button className="text-button" onClick={openLab}>
              View the evidence lab <span>↗</span>
            </button>
          </div>
        </div>
        <div className="signal-card" aria-label="ProjectBench signal display">
          <div className="signal-head">
            <span>LIVE SYSTEMS</span>
            <i></i>
            <span>02</span>
          </div>
          <div className="signal-graph">
            <span className="curve one" />
            <span className="curve two" />
            <span className="curve three" />
            <div className="signal-glow" />
          </div>
          <div className="signal-foot">
            <span>METRICS</span>
            <span>TRACEABLE</span>
            <span>REPEATABLE</span>
          </div>
        </div>
      </section>
      <section className="manifesto">
        <p className="eyebrow">THE POINT OF VIEW</p>
        <p>
          Good software earns trust twice: once when it feels inevitable, and
          again when it still holds up under pressure.
        </p>
        <div className="manifesto-index">
          <span>01 / DESIGN</span>
          <span>02 / SYSTEMS</span>
          <span>03 / EVIDENCE</span>
        </div>
      </section>
      <section className="work-section" id="projects">
        <div className="section-intro">
          <p className="eyebrow">SELECTED WORK</p>
          <h2>
            Projects with a point
            <br />
            of view—and proof.
          </h2>
          <p>
            Each project is a product story, an engineering system, and an
            evolving performance record.
          </p>
        </div>
        <div className="project-list">
          <article className="project-card canvas">
            <div className="project-visual">
              <div className="canvas-window">
                <span className="window-dot" />
                <span className="window-dot" />
                <span className="window-dot" />
                <div className="canvas-shape circle" />
                <div className="canvas-shape square" />
                <div className="canvas-line" />
              </div>
              <span className="project-number">01</span>
            </div>
            <div className="project-copy">
              <div>
                <p className="eyebrow">COLLABORATIVE CANVAS</p>
                <h3>Canvassync</h3>
              </div>
              <p>
                A shared visual workspace built around fluid, immediate
                collaboration.
              </p>
              <div className="project-tags">
                <span>Realtime</span>
                <span>WebSockets</span>
                <span>Canvas</span>
              </div>
              <div className="evidence-line">
                <span
                  className={
                    hasCanvassync ? "evidence-dot ready" : "evidence-dot"
                  }
                />
                {hasCanvassync
                  ? "Read scenario registered"
                  : "Evidence setup pending"}
                <button onClick={openLab}>See metrics ↗</button>
              </div>
            </div>
          </article>
          <article className="project-card know">
            <div className="project-visual">
              <div className="knowledge-window">
                <div className="knowledge-nav">
                  <span />
                  <span />
                  <span />
                </div>
                <div className="knowledge-lines">
                  <i />
                  <i />
                  <i />
                  <i />
                </div>
                <div className="knowledge-orb" />
              </div>
              <span className="project-number">02</span>
            </div>
            <div className="project-copy">
              <div>
                <p className="eyebrow">KNOWLEDGE WORKSPACE</p>
                <h3>Knowdex</h3>
              </div>
              <p>
                An AI-forward space for turning fragmented knowledge into useful
                context.
              </p>
              <div className="project-tags">
                <span>AI</span>
                <span>Search</span>
                <span>Workspace</span>
              </div>
              <div className="evidence-line">
                <span
                  className={hasKnowdex ? "evidence-dot ready" : "evidence-dot"}
                />
                {hasKnowdex
                  ? "Read scenario registered"
                  : "Evidence setup pending"}
                <button onClick={openLab}>See metrics ↗</button>
              </div>
            </div>
          </article>
          <article className="project-card bench">
            <div className="project-visual">
              <div className="bench-chart">
                <span />
                <span />
                <span />
                <span />
                <span />
                <span />
                <span />
              </div>
              <span className="project-number">03</span>
            </div>
            <div className="project-copy">
              <div>
                <p className="eyebrow">PERFORMANCE EVIDENCE</p>
                <h3>ProjectBench</h3>
              </div>
              <p>
                The system behind this portfolio: controlled tests, durable
                reports, and honest comparisons.
              </p>
              <div className="project-tags">
                <span>Rust</span>
                <span>k6</span>
                <span>PostgreSQL</span>
              </div>
              <div className="evidence-line">
                <span className="evidence-dot ready" />
                Control room online<button onClick={openLab}>Open lab ↗</button>
              </div>
            </div>
          </article>
        </div>
      </section>
      <section className="method" id="method">
        <div>
          <p className="eyebrow">HOW THE EVIDENCE WORKS</p>
          <h2>Every number keeps its context.</h2>
        </div>
        <div className="method-steps">
          <div>
            <b>01</b>
            <h3>Define</h3>
            <p>
              Versioned journeys, allowed targets, clear workloads, and
              runtime-only secrets.
            </p>
          </div>
          <div>
            <b>02</b>
            <h3>Measure</h3>
            <p>
              Controlled virtual users produce repeatable latency, throughput,
              and failure data.
            </p>
          </div>
          <div>
            <b>03</b>
            <h3>Compare</h3>
            <p>
              Baselines make regressions legible, with the scenario and
              environment attached.
            </p>
          </div>
        </div>
      </section>
      <section className="evidence-preview">
        <div>
          <p className="eyebrow">CURRENT EVIDENCE</p>
          <h2>
            {loading
              ? "Loading project registry…"
              : `${projects.length} project${projects.length === 1 ? "" : "s"} in the evidence registry`}
          </h2>
          <p>
            Measured reports appear here after approved scenarios run. Until
            then, the system shows setup status instead of made-up performance
            claims.
          </p>
        </div>
        <button onClick={openLab}>
          Enter private lab <span>↗</span>
        </button>
      </section>
      <footer>
        <span>© {new Date().getFullYear()} Krishna Mehta</span>
        <span>Built with intent. Measured with care.</span>
      </footer>
    </>
  );
}

function RegistryPanel({
  projects,
  onChanged,
}: {
  projects: ProjectSummary[];
  onChanged: () => void;
}) {
  const [projectId, setProjectId] = useState(projects[0]?.id ?? "");
  const [projectName, setProjectName] = useState("");
  const [projectDescription, setProjectDescription] = useState("");
  const [environmentName, setEnvironmentName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [scenarioName, setScenarioName] = useState("");
  const [scenarioVersion, setScenarioVersion] = useState("1");
  const [scriptPath, setScriptPath] = useState("");
  const [routes, setRoutes] = useState("");
  const [secretRefs, setSecretRefs] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setMessage(null);
    try {
      let activeProjectId = projectId;
      if (projectName.trim()) {
        const created = await api.createProject({
          name: projectName.trim(),
          description: projectDescription.trim(),
        });
        activeProjectId = created.id;
        setProjectId(activeProjectId);
      }
      if (!activeProjectId) throw new Error("Create or select a project first");
      if (environmentName.trim())
        await api.createEnvironment(activeProjectId, {
          name: environmentName.trim(),
          base_url: baseUrl.trim(),
          allowed_origins: [baseUrl.trim()],
        });
      if (scenarioName.trim())
        await api.createScenario(activeProjectId, {
          name: scenarioName.trim(),
          version: scenarioVersion.trim(),
          script_path: scriptPath.trim(),
          config_snapshot: {
            routes: routes
              .split("\n")
              .map((route) => route.trim())
              .filter(Boolean),
          },
          secret_refs: secretRefs
            .split(",")
            .map((value) => value.trim())
            .filter(Boolean),
        });
      setMessage("Registry saved. Reloading targets…");
      onChanged();
    } catch (cause) {
      setMessage(
        cause instanceof Error ? cause.message : "Configuration failed",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel registry" id="scenarios">
      <p className="eyebrow">TARGET REGISTRY</p>
      <h2>Configure projects, environments, and immutable scenarios</h2>
      <p className="muted">
        Use HTTPS origins and read-only routes. Secret values never belong in
        this form; only uppercase runtime secret references are stored.
      </p>
      <form className="run-form" onSubmit={(event) => void submit(event)}>
        <label>
          Existing project
          <select
            value={projectId}
            onChange={(event) => setProjectId(event.target.value)}
          >
            <option value="">Create a new project below</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name}
              </option>
            ))}
          </select>
        </label>
        <div className="form-grid">
          <label>
            New project name
            <input
              value={projectName}
              onChange={(event) => setProjectName(event.target.value)}
              placeholder="Knowdex"
            />
          </label>
          <label>
            Description
            <input
              value={projectDescription}
              onChange={(event) => setProjectDescription(event.target.value)}
              placeholder="Synthetic read evidence"
            />
          </label>
        </div>
        <div className="form-grid">
          <label>
            Environment name
            <input
              value={environmentName}
              onChange={(event) => setEnvironmentName(event.target.value)}
              placeholder="production-read"
            />
          </label>
          <label>
            HTTPS base URL
            <input
              value={baseUrl}
              onChange={(event) => setBaseUrl(event.target.value)}
              placeholder="https://api.example.com"
            />
          </label>
        </div>
        <div className="form-grid">
          <label>
            Scenario name
            <input
              value={scenarioName}
              onChange={(event) => setScenarioName(event.target.value)}
              placeholder="Read journey"
            />
          </label>
          <label>
            Version
            <input
              value={scenarioVersion}
              onChange={(event) => setScenarioVersion(event.target.value)}
            />
          </label>
        </div>
        <label>
          Script path
          <input
            value={scriptPath}
            onChange={(event) => setScriptPath(event.target.value)}
            placeholder="k6/scenarios/knowdex-read.js"
          />
        </label>
        <label>
          Approved routes, one per line
          <textarea
            value={routes}
            onChange={(event) => setRoutes(event.target.value)}
            placeholder="POST /auth/login&#10;GET /auth/me"
          />
        </label>
        <label>
          Runtime secret references, comma-separated
          <input
            value={secretRefs}
            onChange={(event) => setSecretRefs(event.target.value)}
            placeholder="TEST_ACCOUNT_EMAIL,TEST_ACCOUNT_PASSWORD"
          />
        </label>
        <div>
          <button type="submit" disabled={busy}>
            {busy ? "Saving…" : "Save registry"}
          </button>
        </div>
        {message && <p className="muted">{message}</p>}
      </form>
    </section>
  );
}

function ComparisonPanel({ runs }: { runs: RunSummary[] }) {
  const completed = runs.filter((run) => run.report_available);
  const [baselineId, setBaselineId] = useState("");
  const [candidateId, setCandidateId] = useState("");
  const [comparison, setComparison] = useState<
    import("./api").CompareResponse | null
  >(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!baselineId && completed[1]) setBaselineId(completed[1].id);
    if (!candidateId && completed[0]) setCandidateId(completed[0].id);
  }, [completed, baselineId, candidateId]);
  useEffect(() => {
    if (!baselineId || !candidateId || baselineId === candidateId) return;
    api
      .compare(baselineId, candidateId)
      .then(setComparison)
      .catch((cause: Error) => setError(cause.message));
  }, [baselineId, candidateId]);
  if (completed.length < 2)
    return (
      <section className="panel">
        <p className="eyebrow">COMPARE</p>
        <h2>Comparison needs two reports</h2>
        <p>
          Run two completed measurements before comparing. Missing reports are
          not treated as zero.
        </p>
      </section>
    );
  return (
    <section className="panel comparison">
      <p className="eyebrow">COMPARE EVIDENCE</p>
      <h2>Baseline versus candidate</h2>
      <div className="compare-selects">
        <label>
          Baseline
          <select
            value={baselineId}
            onChange={(event) => setBaselineId(event.target.value)}
          >
            {completed.map((run) => (
              <option key={run.id} value={run.id}>
                {run.id.slice(0, 8)} · {run.created_at}
              </option>
            ))}
          </select>
        </label>
        <label>
          Candidate
          <select
            value={candidateId}
            onChange={(event) => setCandidateId(event.target.value)}
          >
            {completed.map((run) => (
              <option key={run.id} value={run.id}>
                {run.id.slice(0, 8)} · {run.created_at}
              </option>
            ))}
          </select>
        </label>
      </div>
      {error && <p className="error-copy">{error}</p>}
      {comparison && (
        <>
          <p className={comparison.compatible ? "pass" : "fail"}>
            {comparison.compatible
              ? "Compatible comparison"
              : "Exploratory comparison only"}
          </p>
          {comparison.incompatibility_reasons.length > 0 && (
            <ul>
              {comparison.incompatibility_reasons.map((reason) => (
                <li key={reason}>{reason}</li>
              ))}
            </ul>
          )}
          <div className="table-wrap">
            <table>
              <caption>
                Metric deltas; positive values are candidate minus baseline
              </caption>
              <thead>
                <tr>
                  <th scope="col">Metric</th>
                  <th scope="col">Baseline</th>
                  <th scope="col">Candidate</th>
                  <th scope="col">Absolute</th>
                  <th scope="col">Relative</th>
                </tr>
              </thead>
              <tbody>
                {comparison.metrics.map((metric) => (
                  <tr key={metric.name}>
                    <th scope="row">{metric.name}</th>
                    <td>{metric.baseline ?? "—"}</td>
                    <td>{metric.candidate ?? "—"}</td>
                    <td>{metric.absolute?.toFixed(3) ?? "—"}</td>
                    <td>
                      {metric.relative_percent === null
                        ? "—"
                        : `${metric.relative_percent.toFixed(2)}%`}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {comparison.limitations.map((limitation) => (
            <p className="muted" key={limitation}>
              {limitation}
            </p>
          ))}
        </>
      )}
    </section>
  );
}

function ReportDetail({
  run,
  canceling,
  onCancel,
}: {
  run: RunDetail;
  canceling: boolean;
  onCancel: () => Promise<void>;
}) {
  const report = run.report;
  const [buckets, setBuckets] = useState<TimeBucket[]>([]);
  const [bucketError, setBucketError] = useState<string | null>(null);
  useEffect(() => {
    api
      .timeSeries(run.id)
      .then(setBuckets)
      .catch((cause: Error) => setBucketError(cause.message));
  }, [run.id]);
  const peakRequests = Math.max(
    1,
    ...buckets.map((bucket) => bucket.request_count),
  );
  return (
    <>
      <div className="report-title">
        <div>
          <h2>{report?.scenario.name ?? "Run report"}</h2>
          <p>
            {run.id} · {run.target_origin}
          </p>
        </div>
        <div className="status-actions">
          <span className={`status status-${run.status}`}>
            {formatRunStatus(run.status)}
          </span>
          {canCancelRun(run.status) && (
            <button
              className="cancel-button"
              onClick={() => void onCancel()}
              disabled={canceling}
            >
              {canceling ? "Canceling…" : "Cancel run"}
            </button>
          )}
        </div>
      </div>
      <div className="lifecycle-meta">
        <span>Created {new Date(run.created_at).toLocaleString()}</span>
        {run.started_at && (
          <span>Started {new Date(run.started_at).toLocaleString()}</span>
        )}
        {run.finished_at && (
          <span>Finished {new Date(run.finished_at).toLocaleString()}</span>
        )}
      </div>
      {!report && (
        <>
          <p>
            This run has no persisted report yet. It may still be queued or
            running, or it may have failed or been canceled before report
            creation.
          </p>
          {run.failure_reason && (
            <p className="failure-reason">
              <strong>Reason:</strong> {run.failure_reason}
            </p>
          )}
        </>
      )}
      {report && (
        <>
          <div className="metric-grid">
            {[
              ["Requests", report.metrics.request_count.toLocaleString()],
              [
                "Throughput",
                `${report.metrics.throughput_per_second.toFixed(2)}/s`,
              ],
              [
                "Error rate",
                `${(report.metrics.error_rate * 100).toFixed(2)}%`,
              ],
              [
                "p95",
                report.metrics.p95_ms === null
                  ? "—"
                  : `${report.metrics.p95_ms.toFixed(1)} ms`,
              ],
            ].map(([label, value]) => (
              <div className="metric" key={label}>
                <small>{label}</small>
                <strong>{value}</strong>
              </div>
            ))}
          </div>
          <div className="thresholds">
            <h3>Thresholds</h3>
            {report.thresholds.map((threshold) => (
              <p key={threshold.name}>
                <span className={threshold.passed ? "pass" : "fail"}>
                  {threshold.passed ? "PASS" : "FAIL"}
                </span>{" "}
                {threshold.name}
              </p>
            ))}
          </div>
          <div className="report-meta">
            <p>
              <strong>Synthetic k6 traffic.</strong> This is controlled
              virtual-user evidence, not real-user or browser-rendering data.
            </p>
            <p>
              Scenario {report.scenario.version} · {report.workload.vus} VUs ·{" "}
              {report.workload.duration_seconds}s
            </p>
            {run.failure_reason && (
              <p className="failure-reason">
                <strong>Reason:</strong> {run.failure_reason}
              </p>
            )}
          </div>
          <section className="timeseries" aria-labelledby="timeseries-heading">
            <h3 id="timeseries-heading">Measured time series</h3>
            {bucketError && (
              <p className="muted">Time series unavailable: {bucketError}</p>
            )}
            {!bucketError && buckets.length === 0 && (
              <p className="muted">No measured buckets were persisted.</p>
            )}
            {buckets.length > 0 && (
              <>
                <div
                  className="bucket-bars"
                  role="img"
                  aria-label="Request count by ten-second measured bucket"
                >
                  {buckets.map((bucket) => (
                    <div
                      className="bucket-bar"
                      key={bucket.start_at}
                      title={`${new Date(bucket.start_at).toLocaleTimeString()}: ${bucket.request_count} requests`}
                    >
                      <span
                        style={{
                          height: `${Math.max(4, (bucket.request_count / peakRequests) * 100)}%`,
                        }}
                      />
                    </div>
                  ))}
                </div>
                <div className="table-wrap">
                  <table>
                    <caption>Ten-second measured buckets</caption>
                    <thead>
                      <tr>
                        <th scope="col">Start</th>
                        <th scope="col">Requests</th>
                        <th scope="col">Failed</th>
                        <th scope="col">Avg latency</th>
                      </tr>
                    </thead>
                    <tbody>
                      {buckets.map((bucket) => (
                        <tr key={bucket.start_at}>
                          <td>
                            {new Date(bucket.start_at).toLocaleTimeString()}
                          </td>
                          <td>{bucket.request_count}</td>
                          <td>{bucket.failed_count}</td>
                          <td>
                            {bucket.duration_count
                              ? `${(bucket.duration_sum_ms / bucket.duration_count).toFixed(1)} ms`
                              : "—"}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </>
            )}
          </section>
          <div className="artifact-links">
            <h3>Artifacts</h3>
            {report.artifacts.map((artifact) => (
              <a
                key={artifact.kind}
                href={api.artifactUrl(run.id, artifact.kind)}
                target="_blank"
                rel="noreferrer"
              >
                Download {artifact.kind} →
              </a>
            ))}
          </div>
        </>
      )}
    </>
  );
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
