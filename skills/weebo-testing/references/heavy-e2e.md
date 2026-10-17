# Heavy end-to-end tests (real clients)

**Trigger:** external tools consume the service: package managers, IDEs, a browser. A route can be
present, tested and answering `200` with a document that no client can use. In that case only the
real client can catch the bug.

## Layout

```
tests/heavy/
├── <suite>.sh             one per client or concern (npm.sh, cargo.sh, authz.sh, airgap.sh)
├── config.<suite>.toml    the server config that suite starts with
└── lib.sh                 shared: start server, wait for health, cleanup trap, assert helpers
```

## A suite

1. Build and start the server with `config.<suite>.toml` on a random free port. A `trap` on EXIT
   kills it.
2. Point the real client at it (`npm --registry`, `CARGO_REGISTRIES_*`, `pip --index-url`).
3. Run what a user would run: install, publish, resolve.
4. Assert on the client's own result (exit code, installed files) **and** on the server's wire
   transcript: what was fetched, what was refused.
5. Exit non-zero on the first failed assertion, naming it.

Browser suites drive Chrome over CDP (`CDP_URL`, or `CHROME_BIN` to launch one). In a CDE, use the
browser sidecar.

## Running

- `task test:<suite>-heavy` runs one suite. `task test:heavy` runs them all, minus the slowest
  ones, which get their own task.
- CI uses a matrix with one job per suite (`heavy-<suite>`), on every PR and **nightly**. The clients
  and upstreams change without any commit here, so a green tree can turn red overnight.

## Cross-cutting suites

Some suites run one phase per variant: `closed_world.sh` (nothing escapes to the internet),
`authz.sh` (each grant is refused without its token), `airgap.sh`. A new variant gets a phase in
each of them, and a gate test (unit-integration.md §4) fails until it has one.
