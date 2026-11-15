import http from "k6/http";
import { check, sleep } from "k6";

const rawBaseUrl = __ENV.TARGET_ORIGIN;
const credential = __ENV.TEST_ACCOUNT_CREDENTIAL;
const password = __ENV.TEST_ACCOUNT_PASSWORD;
const workspaceId = __ENV.SEEDED_WORKSPACE_ID;

function normalizeOrigin(value) {
  if (!value || !/^https:\/\/[^/?#]+\/?$/.test(value)) {
    throw new Error("TARGET_ORIGIN must be an HTTPS origin");
  }
  return value.replace(/\/$/, "");
}

const baseUrl = normalizeOrigin(rawBaseUrl);

function approved(response) {
  return response.url === baseUrl || response.url.startsWith(`${baseUrl}/`);
}

function bearer(token) {
  return { headers: { Authorization: `Bearer ${token}` }, redirects: 0 };
}

export const options = {
  vus: Number(__ENV.VUS || 1),
  duration: __ENV.DURATION || "60s",
  thresholds: { http_req_failed: ["rate<0.05"] },
};

export function setup() {
  if (!baseUrl || !credential || !password || !workspaceId) {
    throw new Error("Required runtime variables are missing");
  }
  const response = http.post(
    `${baseUrl}/api/v1/auth/login`,
    JSON.stringify({ credential, password }),
    { headers: { "Content-Type": "application/json" }, redirects: 0 },
  );
  const body = response.json();
  check(response, {
    "sign-in succeeded on approved origin":
      (r) => r.status === 200 && approved(r) && body?.success === true,
    "sign-in returned a bearer token": () => Boolean(body?.data?.token),
  });
  if (response.status !== 200 || !body?.data?.token) {
    throw new Error(`Knowdex sign-in failed with status ${response.status}`);
  }
  return { token: body.data.token };
}

export default function (data) {
  const params = bearer(data.token);
  const me = http.get(`${baseUrl}/api/v1/auth/me`, params);
  const workspaces = http.get(`${baseUrl}/api/v1/workspaces`, params);
  const content = http.get(
    `${baseUrl}/api/v1/content?workspaceId=${encodeURIComponent(workspaceId)}`,
    params,
  );
  check(me, {
    "current user read succeeded":
      (r) => r.status === 200 && approved(r),
  });
  check(workspaces, {
    "workspace list read succeeded":
      (r) => r.status === 200 && approved(r),
  });
  check(content, {
    "seeded workspace content read succeeded":
      (r) => r.status === 200 && approved(r),
  });
  sleep(Number(__ENV.THINK_TIME_SECONDS || 2));
}
