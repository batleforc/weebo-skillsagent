# Frontend client: HeyApi with axios

Templates: `templates/ui/openapi-ts.config.ts`, `templates/ui/src/clientInit.ts`.

## 1. Generation

`ui/openapi-ts.config.ts`:

```ts
export default defineConfig({
  input: process.env.OPENAPI_SPEC ?? "openapi.json",   // or a running server's URL
  output: "src/client",
  plugins: ["@hey-api/typescript", "@hey-api/sdk", "@hey-api/client-axios"],
});
```

`ui/package.json` has `"generate": "openapi-ts"`; `task ui:generate` runs it in `ui/`, and
`task generate` runs `dump-spec` first. Output, all gitignored under `ui/src/client/`:

| File | Holds |
| --- | --- |
| `types.gen.ts` | one type per `ToSchema` DTO, plus `<Op>Data`/`<Op>Responses`/`<Op>Errors` per operation |
| `sdk.gen.ts` | one function per operation, named from the `operationId` (`get_item` → `getItem`) |
| `client.gen.ts` | the shared `client` instance |
| `client/`, `core/` | the bundled axios client runtime |

Import through the `@/` alias: `import { getItem } from "@/client/sdk.gen"`,
`import type { ItemResponse } from "@/client/types.gen"`. Never hand-write a mirror of a DTO; if a
type is missing or `unknown`, fix the annotation (the contract test points at it).

## 2. Axios vs batlehub's fetch client

House rule is the axios plugin. Batlehub ships `@hey-api/client-fetch`. Switching is three changes:

| | fetch (batlehub) | axios (house) |
| --- | --- | --- |
| plugin | `@hey-api/client-fetch` | `@hey-api/client-axios` |
| runtime dep | none | `axios` in `dependencies` |
| base URL | `client.setConfig({ baseUrl })` | `client.setConfig({ baseURL })` (axios' spelling; `baseUrl` is a type error) |

`auth` is the same in both: `client.setConfig({ auth: token })` and the SDK sends
`Authorization: Bearer <token>` on operations that declare `security(("bearer_token" = []))`.

## 3. Initialisation

`ui/src/clientInit.ts` is the **first import in `main.ts`**, so the base URL is set before any
module fires a request (batlehub's identity fetch starts at import time). `VITE_API_BASE_URL` carries
the API origin **without a trailing slash**: the SDK appends `/api/v1/…`, so a trailing slash
doubles it in every request. Batlehub's `ui:dev` strips it with `sed 's#/$##'`.

Batlehub's `useAuth` composable sets `auth` on login, refresh and logout; the template exposes
`setApiToken` for the same job.

## 4. Pitfalls seen

- `typescript@7` crashes `@hey-api/openapi-ts` 0.99 (`reading 'AnyKeyword'`); stay on `^6`.
- Any CI job that type-checks, builds or tests the UI must run `pnpm run generate` first, or
  `@/client/*` does not resolve. A job that measures the rendered UI can generate from the committed
  spec; a job that tests behaviour against the API regenerates the spec from source first (see
  [drift-gate.md §3](drift-gate.md)).
