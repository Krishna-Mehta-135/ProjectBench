function normalizeOrigin(value) {
  if (!value || !/^https:\/\/[A-Za-z0-9.-]+(?::[0-9]+)?\/?$/.test(value)) {
    throw new Error("An HTTPS origin is required");
  }
  return value.replace(/\/$/, "");
}

export function validateReadRun(env) {
  const baseUrl = normalizeOrigin(env.TARGET_ORIGIN);
  const approvedOrigin = normalizeOrigin(env.APPROVED_TARGET_ORIGIN);
  if (baseUrl !== approvedOrigin) {
    throw new Error("Target does not match the approved origin");
  }
  if (/^https:\/\/(?:[A-Za-z0-9-]+\.)*canvassync\.tech(?::[0-9]+)?$/.test(baseUrl)) {
    throw new Error("Production CanvasSync origins are forbidden");
  }
  if (!env.TEST_ACCOUNT_EMAIL || !env.TEST_ACCOUNT_PASSWORD || !env.SEEDED_ROOM_ID) {
    throw new Error("Dedicated account and seeded room are required");
  }
  if (!/^[A-Za-z0-9_-]+$/.test(env.SEEDED_ROOM_ID)) {
    throw new Error("Seeded room ID must be a single path segment");
  }
  return {
    baseUrl,
    email: env.TEST_ACCOUNT_EMAIL,
    password: env.TEST_ACCOUNT_PASSWORD,
    seededRoomId: env.SEEDED_ROOM_ID,
  };
}

export function requireApprovedResponse(response, baseUrl) {
  const success = response.status >= 200 && response.status < 300;
  const approvedUrl = response.url === baseUrl || response.url?.startsWith(`${baseUrl}/`);
  if (!success || !approvedUrl) {
    throw new Error("Request did not receive a successful response on the approved origin");
  }
}
