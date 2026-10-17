/**
 * source: batlehub@20879e5 (perf/k6/scenarios/02_warm_read.js), generalised.
 *
 * Scenario 01 — Warm Read Load
 *
 * Reads one pre-seeded resource across a ramp 10 VU → 200 VU → 0. After the
 * first request it is served from cache, so this measures the hot path.
 *
 * Approximate RPS mapping (single core laptop, adjust for your hardware):
 *   10 VU ≈ 100 req/s · 50 VU ≈ 500 · 100 VU ≈ 1000 · 200 VU ≈ 2000
 *
 * Pre-requisite: `task perf:seed`.
 * Run: `task perf:run:read` (thresholds enforced).
 */
import http from "k6/http";
import { check } from "k6";
import { BASE_URL, SEED_PATH } from "../config.js";
import { authHeaders } from "../helpers.js";

export const options = {
  stages: [
    { duration: "30s", target: 10 }, // warm up
    { duration: "60s", target: 50 },
    { duration: "60s", target: 100 },
    { duration: "60s", target: 200 },
    { duration: "30s", target: 0 }, // cool down
  ],
  thresholds: {
    // Error rate below 1 %: hard requirement, in every scenario.
    http_req_failed: ["rate<0.01"],
    // Latency targets: enforced by `perf:run:read`, report-only in `perf:run:all`.
    http_req_duration: ["p(95)<500", "p(99)<2000"],
  },
};

const URL = `${BASE_URL}${SEED_PATH}`;
const HEADERS = authHeaders();

export default function warm_read() {
  const res = http.get(URL, { headers: HEADERS });
  check(res, {
    "status 200": (r) => r.status === 200,
    "body non-empty": (r) => r.body.length > 0,
  });
}
