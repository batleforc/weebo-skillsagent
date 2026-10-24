# Multi-stage build and cache strategy

## 1. Rust: cargo-chef dependency layer

Always use [cargo-chef](https://github.com/LukeMathWalker/cargo-chef) for a Rust image. Docker
caches a layer until one of its inputs changes; cargo-chef splits the build so that a source
edit, or a release version bump, does not recompile the dependency tree:

1. `chef`: the Rust base + system build deps + `cargo install --locked cargo-chef@<exact>`.
2. `planner` (from `chef`): copy the workspace, `cargo chef prepare --recipe-path recipe.json`.
   The recipe holds the manifests and lockfile with the workspace version normalised, so it only
   changes when dependencies or the member list change.
3. `builder` (from `chef`): `COPY --from=planner recipe.json`, then
   `cargo chef cook --locked --release -p <bins> --recipe-path recipe.json`. Cached while the
   recipe is unchanged. Use the same profile, target and features as the final build, or cook
   compiles the wrong artefacts.
4. `COPY` the real sources, `cargo build --locked --release -p <bins>`: only workspace crates
   recompile.

This replaces the hand-written stub crates (one `COPY` + one stub per member, a `touch` for
mtimes, a `; exit 0` that hid failures): a new member needs no Containerfile edit, and a failed
cook fails the build. `cargo-chef` is not a `FROM`, so updatecli does not move it: bump its pin
with the Rust image.

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

The release build of the image took 3 h 04 for v1.5.0; the native binary job, 19 min. Two
causes are fixed in the templates by cargo-chef (§1): the dependency layer invalidated by every
version bump, and the stub build whose failure `; exit 0` hid. Remaining likely causes, none
measured yet, with the retirement batlehub recorded:

| Cause | Fix when it matters |
| --- | --- |
| arm64 `cargo build` runs under QEMU | native arm64 runner per platform joined into a manifest, or `--platform=$BUILDPLATFORM` + a cross target in `builder` |
| `ui-builder` waits for the Rust build to run `dump-spec` | build the UI from the committed spec; check spec drift in CI instead |
| The server compiles once per image and again for the release binary | one shared builder, or pass the built binary in |
| Two images on the default `type=gha` scope evict each other | a `scope=<image>` per image (the templates already do this) |
