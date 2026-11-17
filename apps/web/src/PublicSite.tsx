import { useEffect, useState } from "react";
import "./public-site.css";

type Outcome =
  "verified" | "revised" | "conditional" | "not_supported" | "inconclusive";
type Summary = { median: number; min: number; max: number; count: number };
type Evidence = {
  schema_version: string;
  claim: {
    id: string;
    project: string;
    statement: string;
    metric: string;
    metric_definition: string;
  };
  outcome: Outcome;
  tolerance_percent: number | null;
  reproduce_command: string | null;
  workloads: Array<{
    name: string;
    outcome: Outcome;
    baseline: Summary | null;
    candidate: Summary | null;
    relative_change_percent: number | null;
  }>;
  samples: Array<{
    variant: string;
    workload: string;
    repetition: number;
    value: number;
    unit: string;
    started_at: string;
  }>;
  environment: {
    commit: string;
    images: Record<string, string>;
    host_cpu: string;
    sut_cpuset: string;
    sut_memory_mb: number;
    generator_cpu_peak_percent: number;
    docker: string;
    os: string;
  };
  limitations: string[];
};
type Entry = {
  id: string;
  project: string;
  outcome: Outcome;
  date: string;
  document: Evidence;
};
type Index = { schema_version: string; claims: Array<Omit<Entry, "document">> };
const projects: Record<
  string,
  { name: string; description: string; source: string }
> = {
  canvassync: {
    name: "CanvasSync",
    description: "A collaborative visual workspace with shared room state.",
    source: "https://github.com/Krishna-Mehta-135/Canvassync",
  },
  knowdex: {
    name: "Knowdex",
    description:
      "A workspace for turning fragmented knowledge into useful context.",
    source: "https://github.com/Krishna-Mehta-135/Knowdex",
  },
  projectbench: {
    name: "ProjectBench",
    description:
      "A claims-first system for reproducible experiments and evidence.",
    source: "https://github.com/Krishna-Mehta-135/ProjectBench",
  },
};
const projectName = (slug: string) => projects[slug]?.name ?? slug;
const fmt = (value: number | null | undefined, unit = "") =>
  value == null ? "—" : `${String(value)}${unit}`;
function Status({ outcome }: { outcome: Outcome }) {
  return (
    <span className={`status status-${outcome}`}>
      {outcome.replaceAll("_", " ")}
    </span>
  );
}
function Header() {
  return (
    <header className="pub-header">
      <a className="pub-brand" href="/">
        PB <span>ProjectBench</span>
      </a>
      <nav aria-label="Main">
        <a href="/claims">Claims</a>
        <a href="/method">Method</a>
        <a href="https://github.com/Krishna-Mehta-135">Krishna Mehta ↗</a>
      </nav>
    </header>
  );
}
function Footer() {
  return (
    <footer className="pub-footer">
      <span>Built by Krishna Mehta</span>
      <a href="https://github.com/Krishna-Mehta-135/ProjectBench">
        ProjectBench on GitHub ↗
      </a>
      <a href="https://github.com/Krishna-Mehta-135">Contact ↗</a>
    </footer>
  );
}

function ClaimDetails({ row }: { row: Entry }) {
  const [copied, setCopied] = useState(false);
  const { document: d } = row;
  const e = d.environment;
  const samples = d.samples;
  const values = samples.map((s) => s.value);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const workloads = [...new Set(samples.map((s) => s.workload))];
  const variants = [...new Set(samples.map((s) => s.variant))];
  const w = 760;
  const x = (v: number) => 55 + ((v - min) / span) * 650;
  const rh = 48;
  const h = Math.max(112, workloads.length * variants.length * rh + 36);
  return (
    <>
      <p className="pub-kicker">{projectName(row.project)} / CLAIM EVIDENCE</p>
      <h1>{d.claim.statement}</h1>
      <div className="pub-meta">
        <Status outcome={d.outcome} />
        <span>Updated {new Date(row.date).toLocaleDateString()}</span>
      </div>
      <section className="pub-card">
        <h2>Measured results</h2>
        <p>
          <b>Metric:</b> {d.claim.metric}
        </p>
        <p>{d.claim.metric_definition}</p>
        <p>
          <b>Verification tolerance:</b> {fmt(d.tolerance_percent, "%")}
        </p>
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>Workload</th>
                <th>Outcome</th>
                <th>Baseline median (range)</th>
                <th>Candidate median (range)</th>
                <th>Change</th>
              </tr>
            </thead>
            <tbody>
              {d.workloads.map((a) => (
                <tr key={a.name}>
                  <td>{a.name}</td>
                  <td>
                    <Status outcome={a.outcome} />
                  </td>
                  <td>
                    {a.baseline
                      ? `${fmt(a.baseline.median)} (${fmt(a.baseline.min)}–${fmt(a.baseline.max)})`
                      : "—"}
                  </td>
                  <td>
                    {a.candidate
                      ? `${fmt(a.candidate.median)} (${fmt(a.candidate.min)}–${fmt(a.candidate.max)})`
                      : "—"}
                  </td>
                  <td>{fmt(a.relative_change_percent, "%")}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
      <section className="pub-card">
        <h2>Every repetition</h2>
        <p>
          Horizontal axis: {d.claim.metric} (
          {samples[0]?.unit ?? "unit unavailable"}).
        </p>
        {samples.length ? (
          <>
            <div className="plot-scroll">
              <svg
                viewBox={`0 0 ${w} ${h}`}
                role="img"
                aria-label={`Dot plot of every published sample grouped by workload and variant; horizontal axis in ${samples[0]?.unit ?? "unknown units"}`}
              >
                {[0, 0.25, 0.5, 0.75, 1].map((f) => (
                  <g key={f}>
                    <line
                      x1={55 + f * 650}
                      x2={55 + f * 650}
                      y1="8"
                      y2={h - 25}
                      className="plot-grid"
                    />
                    <text
                      x={55 + f * 650}
                      y={h - 6}
                      textAnchor="middle"
                      className="plot-tick"
                    >
                      {fmt(min + span * f)}
                    </text>
                  </g>
                ))}
                {workloads.flatMap((workload, wi) =>
                  variants.map((variant, vi) => {
                    const y = 26 + (wi * variants.length + vi) * rh;
                    return (
                      <g key={`${workload}-${variant}`}>
                        <text x="2" y={y + 4} className="plot-label">
                          {workload} · {variant}
                        </text>
                        {samples
                          .filter(
                            (s) =>
                              s.workload === workload && s.variant === variant,
                          )
                          .map((s, i) => (
                            <circle
                              key={`${s.repetition}-${i}`}
                              cx={x(s.value)}
                              cy={y}
                              r="4"
                              className={`plot-point ${variant}`}
                            >
                              <title>{`${workload}; ${variant}; rep ${s.repetition}: ${fmt(s.value)} ${s.unit}`}</title>
                            </circle>
                          ))}
                      </g>
                    );
                  }),
                )}
              </svg>
            </div>
            <div className="table-scroll">
              <table>
                <caption>
                  Sample values ({samples[0]?.unit ?? "unit unavailable"})
                </caption>
                <thead>
                  <tr>
                    <th>Workload</th>
                    <th>Variant</th>
                    <th>Repetition</th>
                    <th>Value</th>
                    <th>Started</th>
                  </tr>
                </thead>
                <tbody>
                  {samples.map((s, i) => (
                    <tr key={`${s.workload}-${s.variant}-${s.repetition}-${i}`}>
                      <td>{s.workload}</td>
                      <td>{s.variant}</td>
                      <td>{s.repetition}</td>
                      <td>
                        {fmt(s.value)} {s.unit}
                      </td>
                      <td>{new Date(s.started_at).toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </>
        ) : (
          <p>No published samples.</p>
        )}
      </section>
      <div className="pub-two-col">
        <section className="pub-card">
          <h2>Environment</h2>
          <dl>
            <dt>Commit</dt>
            <dd>
              <code>{e.commit}</code>
            </dd>
            <dt>Host</dt>
            <dd>
              {e.host_cpu} · {e.os}
            </dd>
            <dt>Docker</dt>
            <dd>{e.docker}</dd>
            <dt>SUT limits</dt>
            <dd>
              {e.sut_cpuset} · {e.sut_memory_mb} MB
            </dd>
            <dt>Generator CPU peak</dt>
            <dd>{fmt(e.generator_cpu_peak_percent, "%")}</dd>
            <dt>Images</dt>
            <dd>
              {Object.entries(e.images).map(([k, v]) => (
                <span className="digest" key={k}>
                  {k}: <code>{v}</code>
                </span>
              ))}
            </dd>
          </dl>
        </section>
        <section className="pub-card">
          <h2>Limitations</h2>
          <ul>
            {d.limitations.map((x) => (
              <li key={x}>{x}</li>
            ))}
          </ul>
          {d.reproduce_command && (
            <div className="reproduce">
              <p>
                <b>Reproduce</b>
              </p>
              <code>{d.reproduce_command}</code>
              <button
                type="button"
                onClick={() => {
                  void navigator.clipboard
                    .writeText(d.reproduce_command!)
                    .then(() => setCopied(true));
                }}
              >
                {copied ? "Copied" : "Copy command"}
              </button>
            </div>
          )}
        </section>
      </div>
    </>
  );
}

export default function PublicSite() {
  const [rows, setRows] = useState<Entry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    fetch("/evidence/index.json")
      .then((r) =>
        r.status === 404
          ? { schema_version: "1", claims: [] }
          : r.ok
            ? r.json()
            : Promise.reject(new Error(`Index returned ${r.status}`)),
      )
      .then(async (index: Index) => {
        if (index.schema_version !== "1" || !Array.isArray(index.claims))
          throw new Error("Unsupported evidence index");
        const next = await Promise.all(
          index.claims.map(async (item) => {
            if (
              !/^[a-z0-9-]+$/.test(item.id) ||
              !/^[a-z0-9-]+$/.test(item.project)
            )
              throw new Error("Invalid evidence path");
            const r = await fetch(`/evidence/${item.project}/${item.id}.json`);
            if (!r.ok)
              throw new Error(`Evidence ${item.id} returned ${r.status}`);
            const document = (await r.json()) as Evidence;
            if (
              document.schema_version !== "1" ||
              document.claim.id !== item.id ||
              document.claim.project !== item.project
            )
              throw new Error(`Evidence ${item.id} does not match the index`);
            return { ...item, document };
          }),
        );
        if (live) setRows(next);
      })
      .catch((e) => {
        if (live)
          setError(e instanceof Error ? e.message : "Evidence load failed");
      })
      .finally(() => {
        if (live) setLoading(false);
      });
    return () => {
      live = false;
    };
  }, []);
  const path = window.location.pathname.replace(/\/$/, "") || "/";
  const id = path.startsWith("/claims/")
    ? decodeURIComponent(path.slice(8))
    : "";
  const slug = path.startsWith("/projects/")
    ? decodeURIComponent(path.slice(10))
    : "";
  const selected = rows.find((r) => r.id === id);
  const projectRows = rows.filter((r) => r.project === slug);
  let page;
  if (path === "/") {
    const verified = rows.filter(
      (r) => r.document.outcome === "verified",
    ).length;
    const revised = rows.filter((r) => r.document.outcome === "revised").length;
    const measuring = rows.filter((r) =>
      ["inconclusive", "conditional", "not_supported"].includes(
        r.document.outcome,
      ),
    ).length;
    const slugs = [...new Set(rows.map((r) => r.project))];
    page = (
      <>
        <section className="pub-hero">
          <p className="pub-kicker">REPRODUCIBLE SOFTWARE EVIDENCE</p>
          <h1>
            Make engineering claims <em>measurable.</em>
          </h1>
          <p>
            ProjectBench connects a claim to a controlled experiment, a fixed
            verdict, and the evidence behind it.
          </p>
          <a className="pub-button" href="/claims">
            Explore claims →
          </a>
        </section>
        <section className="scoreboard">
          {[
            ["Verified", verified],
            ["Revised", revised],
            ["Measuring", measuring],
          ].map(([name, count]) => (
            <div key={name}>
              <span>{name}</span>
              <strong>{loading ? "—" : count}</strong>
            </div>
          ))}
        </section>
        <section className="pub-section">
          <p className="pub-kicker">PROJECTS</p>
          <h2>Evidence follows the work.</h2>
          {slugs.length ? (
            <div className="pub-projects">
              {slugs.map((s) => (
                <article className="pub-card" key={s}>
                  <p className="pub-kicker">
                    {rows.filter((r) => r.project === s).length} CLAIMS
                  </p>
                  <h3>{projectName(s)}</h3>
                  <p>
                    {projects[s]?.description ??
                      "Software project and its measured claims."}
                  </p>
                  <a href={`/projects/${s}`}>View project →</a>
                </article>
              ))}
            </div>
          ) : (
            <div className="pub-empty">
              <b>
                {loading
                  ? "Loading published evidence…"
                  : "Measurement is in progress."}
              </b>
              <p>
                No claims have published evidence yet. Counts remain zero until
                reviewed evidence is published.
              </p>
            </div>
          )}
        </section>
        <section className="pub-section pub-steps">
          <p className="pub-kicker">THREE STEPS</p>
          <h2>Claim. Measure. Review.</h2>
          <ol>
            <li>Define a claim, metric, and workload.</li>
            <li>Run controlled local experiments.</li>
            <li>Review the verdict and publish allowlisted evidence.</li>
          </ol>
        </section>
        <section className="pub-quick">
          <div>
            <p className="pub-kicker">TRY IT LOCALLY</p>
            <h2>Reproduce a result.</h2>
            <p>
              Start with the ProjectBench quickstart and run the bundled demo.
            </p>
          </div>
          <code>make demo</code>
        </section>
      </>
    );
  } else if (path === "/claims") {
    page = (
      <>
        <p className="pub-kicker">EVIDENCE INDEX</p>
        <h1>Claims</h1>
        {rows.length ? (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>Claim</th>
                  <th>Project</th>
                  <th>Outcome</th>
                  <th>Updated</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((r) => (
                  <tr key={r.id}>
                    <td>
                      <a href={`/claims/${r.id}`}>
                        {r.document.claim.statement}
                      </a>
                    </td>
                    <td>{projectName(r.project)}</td>
                    <td>
                      <Status outcome={r.document.outcome} />
                    </td>
                    <td>{new Date(r.date).toLocaleDateString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="pub-empty">
            <b>
              {loading
                ? "Loading published evidence…"
                : "No published claims yet."}
            </b>
            <p>
              Claims appear here after evidence passes review and is published.
            </p>
          </div>
        )}
      </>
    );
  } else if (id) {
    page = selected ? (
      <ClaimDetails row={selected} />
    ) : (
      <div className="pub-empty">
        <h1>Claim not found</h1>
        <p>
          {loading
            ? "Loading evidence…"
            : "This claim has no published evidence."}
        </p>
        <a href="/claims">Browse claims</a>
      </div>
    );
  } else if (slug) {
    page = (
      <>
        <p className="pub-kicker">PROJECT</p>
        <h1>{projectName(slug)}</h1>
        <p>
          {projects[slug]?.description ??
            "Software project and its measured claims."}
        </p>
        {projects[slug] && (
          <p>
            <a href={projects[slug].source}>Source repository ↗</a>
          </p>
        )}
        {projectRows.length ? (
          projectRows.map((r) => (
            <a
              className="pub-card project-claim"
              href={`/claims/${r.id}`}
              key={r.id}
            >
              <Status outcome={r.document.outcome} />
              <h2>{r.document.claim.statement}</h2>
            </a>
          ))
        ) : (
          <div className="pub-empty">
            No published claims for this project yet.
          </div>
        )}
      </>
    );
  } else if (path === "/method") {
    page = (
      <>
        <p className="pub-kicker">METHODOLOGY</p>
        <h1>How measurements earn trust.</h1>
        <section className="pub-card">
          <h2>Controlled local experiments</h2>
          <p>
            Benchmarks run locally with stated workloads, repetitions, image
            digests, and resource limits. Generator CPU above 70%, mismatched
            environments, too few repetitions, or ranges wider than the median
            difference make a result inconclusive.
          </p>
          <p>
            Comparisons use medians and ranges. Fixed rules compute each
            verdict. Evidence pages show the environment and limitations beside
            every sample.
          </p>
          <p>
            These self-measured results are reproducible evidence, not an
            independent audit or a promise about production behavior.
          </p>
        </section>
      </>
    );
  } else
    page = (
      <div className="pub-empty">
        <h1>Page not found</h1>
        <a href="/">Return home</a>
      </div>
    );
  return (
    <div className="pub-shell">
      <Header />
      <main className="pub-main">
        {error && (
          <div className="pub-error" role="alert">
            <b>Evidence could not be loaded.</b> {error}
          </div>
        )}
        {page}
      </main>
      <Footer />
    </div>
  );
}
