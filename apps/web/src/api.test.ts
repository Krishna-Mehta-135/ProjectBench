import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";

describe("run report API", () => {
  afterEach(() => vi.restoreAllMocks());

  it("loads a run detail without starting execution", async () => {
    const response = { id: "run-1", status: "completed", report: null };
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: () => Promise.resolve(response) }));

    await expect(api.run("run-1")).resolves.toEqual(response);
    expect(fetch).toHaveBeenCalledWith("/api/v1/runs/run-1");
  });

  it("encodes artifact kinds in download URLs", () => {
    expect(api.artifactUrl("run-1", "k6/raw")).toBe("/api/v1/runs/run-1/artifacts/k6%2Fraw");
  });

  it("cancels a run through the explicit operator endpoint", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true }));

    await expect(api.cancelRun("run-1")).resolves.toBeUndefined();
    expect(fetch).toHaveBeenCalledWith("/api/v1/runs/run-1/cancel", { method: "POST" });
  });
});
