import assert from "node:assert/strict";
import test from "node:test";
import { collectHealthObservations } from "./observe.mjs";

test("demo measures generator CPU across the full behavior run", async () => {
  const times = [0, 0, 10, 100, 110, 200, 210, 300, 310, 400, 410, 500];
  const usageCalls = [];
  const observations = await collectHealthObservations({
    iterations: 5,
    request: async () => ({ status: 200, json: async () => ({ status: "ok" }) }),
    sleep: async () => {},
    now: () => times.shift(),
    cpuUsage: (start) => {
      usageCalls.push(start);
      return start ? { user: 100_000, system: 0 } : { user: 0, system: 0 };
    },
    timestamp: () => "2026-11-24T00:00:00.000Z",
  });

  assert.equal(observations.results.length, 5);
  assert.ok(observations.results.every((result) => result.succeeded));
  assert.equal(observations.generatorCpuPercent, 20);
  assert.equal(usageCalls.length, 2, "CPU is sampled once for the complete run");
});
