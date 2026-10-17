// source: batlehub@20879e5 (perf/k6/helpers.js), trimmed.
import { check } from "k6";
import encoding from "k6/encoding";
import http from "k6/http";
import { ADMIN_TOKEN } from "./config.js";

export function authHeaders(token) {
  return { Authorization: `Bearer ${token || ADMIN_TOKEN}` };
}

/** GET with auth, assert status, return response. */
export function getOk(url, params) {
  const res = http.get(url, { headers: authHeaders(), ...params });
  check(res, { "status 200": (r) => r.status === 200 });
  return res;
}

/**
 * Base64 of ~byteLen random bytes. Native on purpose: building bytes in a
 * script loop starves the k6 runner and measures k6 instead of the server.
 */
export function randomBase64(byteLen) {
  const bytes = new Uint8Array(Math.ceil(byteLen / 3) * 3);
  // Web Crypto fills at most 65 536 bytes per call.
  for (let off = 0; off < bytes.length; off += 65536) {
    crypto.getRandomValues(bytes.subarray(off, off + 65536));
  }
  return encoding.b64encode(bytes.buffer);
}
