# TypeScript: resilient and secure

The same rules as [rust.md](rust.md), translated. TS has no ANSSI guide; these lean on the
compiler and typescript-eslint so they are checked, not remembered.

## 1. Setup

`tsconfig.json`:

```jsonc
{
  "compilerOptions": {
    "strict": true,
    "noUncheckedIndexedAccess": true,     // arr[i] is T | undefined: the TS twin of `.get(i)`
    "exactOptionalPropertyTypes": true,
    "noImplicitOverride": true,
    "noFallthroughCasesInSwitch": true,
    "useUnknownInCatchVariables": true     // part of strict, listed so nobody turns it off
  }
}
```

Lint rules, as errors (typescript-eslint names; oxlint carries the same ones):

| Rule | Why |
| --- | --- |
| `no-explicit-any` | `any` switches the checker off for everything it touches |
| `no-non-null-assertion` | `x!` is `unwrap()` without the panic message |
| `consistent-type-assertions` (`assertionStyle: "never"`, `as const` allowed) | `as T` is an unchecked cast |
| `no-floating-promises`, `no-misused-promises` | an un-awaited promise is an ignored `Result` |
| `switch-exhaustiveness-check` | a new enum variant fails the build, not production |
| `ban-ts-comment` (`ts-expect-error` allowed with description ≥ 10 chars) | the TS form of `#[expect(..., reason)]` |
| `eslint-comments/require-description` | a disabled rule says why, like a clippy `reason` |

## 2. Rules

- **Justified escapes only**: `// @ts-expect-error <why>` or
  `// eslint-disable-next-line <rule> -- <why>`, one line, never file-wide. `@ts-ignore` is banned.
- **Validate at the trust boundary**: the generated HeyApi client gives types, not guarantees.
  Parse anything from outside your code (API responses you do not own, `localStorage`, URL params,
  `postMessage`, `JSON.parse`) with a schema (zod / valibot, or HeyApi's schema plugin) into a typed
  value. `JSON.parse(x) as T` is banned by the cast rule for this reason.
- **Errors**: `catch (e)` receives `unknown`; narrow it (`e instanceof ApiError`) before use. Never
  an empty `catch {}`; never `.catch(() => {})`. Show the user a state (error banner, retry), log
  the detail.
- **Every request has a timeout and a cancel**: axios `timeout`, `AbortController` on unmount or
  route change. No polling loop where an event or SSE exists (nr/rules).
- **Exhaustive unions**: `default: { const never: never = x; throw new Error(...) }` — the one place
  a `throw` documents an impossible state.
- **XSS**: no `v-html` / `innerHTML` with data you did not write; if unavoidable, sanitize with
  DOMPurify and say why in a comment. No `eval`, no `new Function`.
- **Secrets never reach the bundle**: anything in `import.meta.env.VITE_*` is public.
