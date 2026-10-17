# Coverage

## Rust: cargo-llvm-cov

`templates/.tasks/coverage.yaml` defines:

- `coverage`: writes `coverage/html/` and `lcov.info` (for Sonar).
- `coverage-check`: `--fail-under-lines 80`.

Both run one internal `cov:collect` task, so they cannot drift apart. It runs
`cargo llvm-cov --no-report` once for the workspace, then once per integration binary that needs
a feature flag or a backend, and the reports merge.

- **Exclusions** are a single regex, `COVERAGE_EXCLUDE`, passed to every invocation. Exclude only
  code that cannot be tested in-process: `main.rs` wiring, live-upstream clients, the TUI. Each
  exclusion gets a comment saying why, and stays in sync with the Sonar exclusions.
- **Real backends**: when the project has §3 tests, `cov:collect` starts the containers
  (in a CDE they are devfile sidecars, see the project CLAUDE.md) and exports the env vars before
  it runs those binaries. A coverage number that silently skipped them is not the real number.
- Add `coverage/` and `lcov.info` to `.gitignore`.

## Frontend: vitest v8

In `vite.config.ts`:

```ts
coverage: {
  provider: "v8",
  reporter: ["text", "lcov", "html"],
  include: coverageIncludeFromTests(),
  thresholds: { lines: 80, branches: 80, functions: 80, statements: 80 },
},
```

`coverageIncludeFromTests()` (batlehub `ui/vite.config.ts`) builds the `include` list from the
co-located `*.test.ts` files. Under `src/components/` a test enrols every sibling source in its
directory; everywhere else `Foo.test.ts` enrols `Foo.vue`/`Foo.ts`. The 80% applies to code that
has tests, and adding a test file enrols its source automatically. The intent: an untested file
does not drag the gate down, but a tested one must be tested properly. Copy the function from
batlehub if the project wants this behaviour; otherwise `include: ["src/**"]` is the stricter
default.

## CI

Each job produces lcov (`cargo llvm-cov --lcov`, `pnpm run coverage`). Sonar, or a merge step,
consumes the files. The 80% gate runs in `task coverage-check` and `ui:coverage`, not only in Sonar,
so it fails locally too.
