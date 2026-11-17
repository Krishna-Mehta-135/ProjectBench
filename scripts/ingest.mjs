import { randomUUID } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";

const directory = process.argv[2];
if (!directory) {
  console.error("usage: node scripts/ingest.mjs <result-directory>");
  process.exit(2);
}

const baseUrl = process.env.PROJECTBENCH_API_URL ?? "http://127.0.0.1:8080";
const resultSetId = process.env.PROJECTBENCH_RESULT_SET_ID ?? randomUUID();
const token = process.env.METRICS_OPERATOR_TOKEN;
const files = (await readdir(directory))
  .filter((name) => name.endsWith(".json") && !name.endsWith("-summary.json"))
  .sort();
if (files.length === 0) {
  console.error(`no result JSON files found in ${directory}`);
  process.exit(2);
}

console.log(`ingesting ${files.length} results in set ${resultSetId}`);

for (const name of files) {
  const contents = await readFile(join(directory, name), "utf8");
  const result = JSON.parse(contents);
  if (result.schema_version !== "1") {
    throw new Error(`${name} is not a schema-v1 result`);
  }
  const headers = {
    "content-type": "application/json",
    "x-projectbench-result-set": resultSetId,
  };
  if (token) headers.authorization = `Bearer ${token}`;
  const response = await fetch(`${baseUrl}/api/v1/results`, {
    method: "POST",
    headers,
    body: contents,
  });
  if (!response.ok) {
    throw new Error(`${name}: ingest failed with HTTP ${response.status}`);
  }
}
console.log(`ingested ${files.length} results in set ${resultSetId}`);
