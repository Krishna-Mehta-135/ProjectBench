import React from "react";
import { createRoot } from "react-dom/client";
import "./style.css";

const cards = [
  ["Canvassync", "Azure staging", "No baseline yet"],
  ["Knowdex", "Azure staging", "Not onboarded"],
];

function App() {
  return <main className="shell">
    <header><div><p className="eyebrow">METRICS / EVIDENCE</p><h1>Performance, with provenance.</h1><p className="lede">A private control plane for reproducible synthetic performance evidence.</p></div><button>New run</button></header>
    <section className="notice"><strong>Synthetic traffic only.</strong> Results from k6 virtual users are not measurements of real users or browser rendering.</section>
    <section className="grid">{cards.map(([name, env, state]) => <article key={name}><div className="card-head"><span className="dot"/><span>{env}</span></div><h2>{name}</h2><p>{state}</p><a href="#scenarios">View scenarios →</a></article>)}</section>
    <section id="scenarios" className="panel"><div><p className="eyebrow">RUN HISTORY</p><h2>Nothing has been run yet</h2><p>Configure a scenario, preview its target and workload, then create the first evidence report.</p></div><dl><div><dt>Projects</dt><dd>2</dd></div><div><dt>Runs</dt><dd>0</dd></div><div><dt>AI telemetry</dt><dd>Deferred</dd></div></dl></section>
  </main>;
}

createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);

