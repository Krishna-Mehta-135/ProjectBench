import { describe, expect, it } from "vitest";
import { canCancelRun, formatRunStatus } from "./runPresentation";

describe("run presentation rules", () => {
  it("allows cancellation only for active states", () => {
    expect(canCancelRun("queued")).toBe(true);
    expect(canCancelRun("running")).toBe(true);
    expect(canCancelRun("completed")).toBe(false);
    expect(canCancelRun("failed")).toBe(false);
    expect(canCancelRun("canceled")).toBe(false);
  });

  it("keeps status labels readable", () => {
    expect(formatRunStatus("canceled")).toBe("canceled");
    expect(formatRunStatus("in_progress")).toBe("in progress");
  });
});
