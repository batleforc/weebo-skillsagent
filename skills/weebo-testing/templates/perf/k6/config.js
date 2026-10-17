// source: batlehub@20879e5 (perf/k6/config.js), trimmed.
// Base URL of the server under test.
export const BASE_URL = __ENV.PERF_URL || "http://localhost:8080";

// Static API token, declared in perf/config.perf.toml for the perf run only.
export const ADMIN_TOKEN = __ENV.PERF_TOKEN || "perf-admin-token";

// Fixtures created by `task perf:seed`.
export const SEED_PATH = __ENV.PERF_SEED_PATH || "/api/health";
