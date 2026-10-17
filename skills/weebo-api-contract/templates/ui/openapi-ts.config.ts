// source: batlehub@20879e5 (ui/openapi-ts.config.ts), trimmed.
// Axios client per the Weebo house rule; batlehub itself ships `@hey-api/client-fetch`.
import { defineConfig } from "@hey-api/openapi-ts";

// Generate the spec file with: task dump-spec (cargo run -p my-server -- dump-spec > ui/openapi.json)
// Or set OPENAPI_SPEC to a URL for a running server, e.g. http://localhost:8080/api/openapi.json
export default defineConfig({
  input: process.env.OPENAPI_SPEC ?? "openapi.json",
  output: "src/client",
  plugins: ["@hey-api/typescript", "@hey-api/sdk", "@hey-api/client-axios"],
});
