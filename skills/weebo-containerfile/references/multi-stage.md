# Multi-stage build and cache strategy

## 1. Rust: the stub-crate dependency layer

Docker caches a layer until one of its inputs changes. The `builder` stage splits the build so
that a source edit does not recompile the dependency tree:

1. `COPY Cargo.toml Cargo.lock` and **every** workspace member's `Cargo.toml` (one line each,
   plus any `[patch]` directory such as batlehub's `patches/`).
2. Write a stub `src/lib.rs` (`pub fn _stub() {}`) or `src/main.rs` (`fn main() {}`) per member.
3. `cargo build --locked --release -p <bins>`: compiles only the dependencies. Cached while the
   manifests and lockfile are unchanged.
4. `COPY` the real sources, `touch` every `lib.rs`/`main.rs` (`COPY` keeps the context's mtimes,
   which can be older than the stub build, and cargo would then skip the real files), build again.

A member missing from steps 1-2 makes step 3 fail, which the template's `; exit 0` hides: the
build still succeeds, uncached. When adding a crate, add its lines.

## 2. Frontend

- Copy `package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml` first, `pnpm install
  --frozen-lockfile`, then `COPY ui/`. Dependencies stay cached until the lockfile changes.
- pnpm is installed with `npm install -g --ignore-scripts pnpm@<exact>`, in sync with the
  `packageManager` field: Corepack is gone from Node 25+.
- batlehub regenerates the API client inside the image: it copies the server binary from
  `builder`, runs `dump-spec > openapi.json`, then `pnpm run generate` and `pnpm run build`
  (contract: weebo-api-contract). `.dockerignore` excludes `ui/openapi.json` and
  `ui/src/client/` so a stale local copy cannot leak in.

## 3. Runtime stage

Copy only artefacts: `target/release/<bin>`, `ui/dist`, and pre-created writable directories.
A distroless runtime has no `mkdir` or `chown`, which is why the builder creates
`/var/cache/<app>` and the runtime copies it with `--chown=65532:65532`. The server is pointed at
the SPA with a config/env value (`…__SERVER__STATIC_DIR=/app/ui/dist` in batlehub's compose).
`ENTRYPOINT` is the binary in exec form; `CMD` holds its default `--config` path, so a deployer
overrides arguments without restating the binary.

## 4. Known debt (batlehub TD-0003)

The release build of the image took 3 h 04 for v1.5.0; the native binary job, 19 min. Likely
causes, none measured yet, with the retirement batlehub recorded:

| Cause | Fix when it matters |
| --- | --- |
| arm64 `cargo build` runs under QEMU | native arm64 runner per platform joined into a manifest, or `--platform=$BUILDPLATFORM` + a cross target in `builder` |
| A release bumps the workspace version, so the dependency layer never matches on the one build that counts | `cargo-chef` (normalises the version) or BuildKit `--mount=type=cache` on `target/` and `~/.cargo/registry` |
| `2>/dev/null; exit 0` hides a failed stub build | let the stub build fail the build |
| `ui-builder` waits for the Rust build to run `dump-spec` | build the UI from the committed spec; check spec drift in CI instead |
| The server compiles once per image and again for the release binary | one shared builder, or pass the built binary in |
| Two images on the default `type=gha` scope evict each other | a `scope=<image>` per image (the templates already do this) |
