import assert from "node:assert/strict";
import test from "node:test";
import { validateReadRun, requireApprovedResponse } from "./canvassync-read-gate.mjs";

const valid = {
  TARGET_ORIGIN: "https://staging.example.test",
  APPROVED_TARGET_ORIGIN: "https://staging.example.test",
  TEST_ACCOUNT_EMAIL: "bench@example.test",
  TEST_ACCOUNT_PASSWORD: "secret-test-only",
  SEEDED_ROOM_ID: "room-1",
};

test("read run requires a separately approved exact HTTPS origin and credentials", () => {
  assert.equal(validateReadRun(valid).baseUrl, valid.TARGET_ORIGIN);
  for (const field of Object.keys(valid)) {
    assert.throws(() => validateReadRun({ ...valid, [field]: "" }), /required|HTTPS/i);
  }
  assert.throws(() => validateReadRun({ ...valid, TARGET_ORIGIN: "https://other.example.test" }), /approved origin/i);
  assert.throws(() => validateReadRun({ ...valid, TARGET_ORIGIN: "http://staging.example.test" }), /HTTPS/i);
  assert.throws(() => validateReadRun({ ...valid, TARGET_ORIGIN: "https://staging.example.test/extra" }), /HTTPS origin/i);
  assert.throws(() => validateReadRun({ ...valid, TARGET_ORIGIN: "https://api.canvassync.tech", APPROVED_TARGET_ORIGIN: "https://api.canvassync.tech" }), /production/i);
  assert.throws(() => validateReadRun({ ...valid, SEEDED_ROOM_ID: "../admin" }), /room ID/i);
});

test("response gate rejects redirects, errors, and a different origin", () => {
  const approved = valid.TARGET_ORIGIN;
  assert.doesNotThrow(() => requireApprovedResponse({ status: 200, url: `${approved}/api/v1/room/mine` }, approved));
  for (const response of [
    { status: 302, url: `${approved}/login` },
    { status: 401, url: `${approved}/api/v1/room/mine` },
    { status: 200, url: "https://other.example.test/api/v1/room/mine" },
    { status: 200, url: `${approved}.evil.test/api/v1/room/mine` },
  ]) {
    assert.throws(() => requireApprovedResponse(response, approved), /approved origin|successful response/i);
  }
});
