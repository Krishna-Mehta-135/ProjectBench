export function canCancelRun(status: string): boolean {
  return status === "queued" || status === "running";
}

export function formatRunStatus(status: string): string {
  return status.replaceAll("_", " ");
}
