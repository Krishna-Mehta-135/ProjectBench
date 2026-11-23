export async function collectHealthObservations({
  iterations,
  request,
  sleep,
  now,
  cpuUsage,
  timestamp,
}) {
  const runStartedAt = now();
  const runStartedUsage = cpuUsage();
  const results = [];

  for (let repetition = 1; repetition <= iterations; repetition += 1) {
    const startedAt = timestamp();
    const requestStartedAt = now();
    const response = await request();
    const body = await response.json();
    const durationSeconds = (now() - requestStartedAt) / 1000;
    results.push({
      repetition,
      startedAt,
      durationSeconds,
      status: response.status,
      succeeded: response.status === 200 && body.status === "ok",
    });
    await sleep(100);
  }

  const elapsedMicros = Math.max(1, (now() - runStartedAt) * 1000);
  const used = cpuUsage(runStartedUsage);
  const generatorCpuPercent = Math.min(
    100,
    ((used.user + used.system) / elapsedMicros) * 100,
  );
  return { results, generatorCpuPercent };
}
