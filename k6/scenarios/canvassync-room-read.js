import http from "k6/http";
import { check, sleep } from "k6";

const rawBaseUrl = __ENV.TARGET_ORIGIN;
const email = __ENV.TEST_ACCOUNT_EMAIL;
const password = __ENV.TEST_ACCOUNT_PASSWORD;
const seededRoomId = __ENV.SEEDED_ROOM_ID;

function normalizeOrigin(value) {
  if (!value || !/^https:\/\/[^/?#]+\/?$/.test(value)) {
    throw new Error("TARGET_ORIGIN must be an HTTPS origin");
  }
  return value.replace(/\/$/, "");
}

const baseUrl = normalizeOrigin(rawBaseUrl);

function stayedOnApprovedOrigin(response) {
  return response.url === baseUrl || response.url.startsWith(`${baseUrl}/`);
}

function readOnlyParams(cookies) {
  return { cookies, redirects: 0 };
}

export const options = { vus: Number(__ENV.VUS || 1), duration: __ENV.DURATION || "60s", thresholds: { http_req_failed: ["rate<0.05"] } };

export function setup() {
  if (!baseUrl || !email || !password || !seededRoomId) throw new Error("Required runtime variables are missing");
  const response = http.post(`${baseUrl}/api/v1/auth/signin`, JSON.stringify({ email, password }), { headers: { "Content-Type": "application/json" }, redirects: 0 });
  check(response, { "sign-in succeeded on approved origin": (r) => r.status >= 200 && r.status < 300 && stayedOnApprovedOrigin(r) });
  return { cookie: response.cookies };
}

export default function (data) {
  const params = readOnlyParams(data.cookie);
  const roomList = http.get(`${baseUrl}/api/v1/room/mine`, params);
  const shapes = http.get(`${baseUrl}/api/v1/room/${seededRoomId}/shapes`, params);
  check(roomList, { "room list read succeeded on approved origin": (r) => r.status >= 200 && r.status < 300 && stayedOnApprovedOrigin(r) });
  check(shapes, { "room shapes read succeeded on approved origin": (r) => r.status >= 200 && r.status < 300 && stayedOnApprovedOrigin(r) });
  sleep(Number(__ENV.THINK_TIME_SECONDS || 2));
}

