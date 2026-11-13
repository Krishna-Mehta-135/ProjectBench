import http from "k6/http";
import { check, sleep } from "k6";

const baseUrl = __ENV.TARGET_ORIGIN;
const email = __ENV.TEST_ACCOUNT_EMAIL;
const password = __ENV.TEST_ACCOUNT_PASSWORD;
const seededRoomId = __ENV.SEEDED_ROOM_ID;

export const options = { vus: Number(__ENV.VUS || 1), duration: __ENV.DURATION || "60s", thresholds: { http_req_failed: ["rate<0.05"] } };

export function setup() {
  if (!baseUrl || !email || !password || !seededRoomId) throw new Error("Required runtime variables are missing");
  const response = http.post(`${baseUrl}/api/v1/auth/signin`, JSON.stringify({ email, password }), { headers: { "Content-Type": "application/json" }, redirects: 0 });
  check(response, { "sign-in succeeded": (r) => r.status >= 200 && r.status < 300 });
  return { cookie: response.cookies };
}

export default function (data) {
  const params = { cookies: data.cookie };
  check(http.get(`${baseUrl}/api/v1/room/mine`, params), { "room list succeeded": (r) => r.status >= 200 && r.status < 300 });
  check(http.get(`${baseUrl}/api/v1/room/${seededRoomId}/shapes`, params), { "shape read succeeded": (r) => r.status >= 200 && r.status < 300 });
  sleep(Number(__ENV.THINK_TIME_SECONDS || 2));
}

