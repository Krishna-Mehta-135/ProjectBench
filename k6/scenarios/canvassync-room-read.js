import http from "k6/http";
import { sleep } from "k6";
import { validateReadRun, requireApprovedResponse } from "./canvassync-read-gate.mjs";

const { baseUrl, email, password, seededRoomId } = validateReadRun(__ENV);

function readOnlyParams(cookies) {
  return { cookies, redirects: 0 };
}

export const options = { vus: Number(__ENV.VUS || 1), duration: __ENV.DURATION || "60s", thresholds: { http_req_failed: ["rate<0.05"] } };

export function setup() {
  const response = http.post(`${baseUrl}/api/v1/auth/signin`, JSON.stringify({ email, password }), { headers: { "Content-Type": "application/json" }, redirects: 0 });
  requireApprovedResponse(response, baseUrl);
  return { cookie: response.cookies };
}

export default function (data) {
  const params = readOnlyParams(data.cookie);
  const roomList = http.get(`${baseUrl}/api/v1/room/mine`, params);
  requireApprovedResponse(roomList, baseUrl);
  const shapes = http.get(`${baseUrl}/api/v1/room/${seededRoomId}/shapes`, params);
  requireApprovedResponse(shapes, baseUrl);
  sleep(Number(__ENV.THINK_TIME_SECONDS || 2));
}

