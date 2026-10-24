---
name: weebo-containerfile
description: House container image practice for Weebo projects (Rust backend + TS frontend), modelled on batlehub — the multi-stage Containerfile (Rust deps cached with cargo-chef, pnpm UI stage, distroless non-root runtime), its Containerfile.hardened twin on Red Hat Hardened Images, digest-pinned bases kept current by updatecli, the .dockerignore, the GitHub Actions jobs that build, scan, sign and attest the image, and the podman compose tasks. Use when writing or reviewing a Containerfile, adding an image to a project, shrinking or speeding up an image build, or wiring image build/push into CI.
---

# Weebo container images

The reference implementation is [batlehub](https://github.com/batleforc/batlehub). Every file
under `templates/` is a snapshot of batlehub at commit `20879e5`, trimmed to one server image.
When a template and this text disagree, the template wins: it is what ran.

## 1. The files

| File | Role |
| --- | --- |
| `Containerfile` | the image CI builds, scans and publishes: Debian/Node builders, `distroless/cc` runtime |
| `Containerfile.hardened` | same stages on Red Hat Hardened Images, for deployers who require them; not built in CI |
| `.dockerignore` | keeps `target/`, `node_modules/`, generated client, `.git/` out of the context |
| `.github/workflows/image-scan.yaml` | PR, main and daily: build (no push) + Trivy gate |
| `.github/workflows/image-release.yaml` | on tag: multi-arch build, push, sign, SBOM + provenance, Trivy gate |
| `.updatecli/manifest/docker.yaml` | moves every `FROM` tag and digest together |
| `.tasks/compose.yaml`, `docker-compose.yml` | local stack under `podman compose` |

Named `Containerfile`, not `Dockerfile`: CI passes `file: ./Containerfile`. Use `.dockerignore`:
docker reads only that name, and podman/buildah read it too when no `.containerignore` exists.

## 2. Stages

| Stage | Base | Does |
| --- | --- | --- |
| `chef` | `rust:<ver>-slim-bookworm` | system build deps + pinned `cargo-chef`; shared by the next two |
| `planner` | `chef` | workspace sources → `cargo chef prepare` → `recipe.json` |
| `builder` | `chef` | `cargo chef cook --locked --release` (dependency layer); then real sources → `cargo build --locked --release` |
| `ui-builder` | `node:<ver>-slim` | pnpm pinned, `--frozen-lockfile`, spec dumped from the built binary → HeyApi client → `vite build` |
| `runtime` | `gcr.io/distroless/cc-debian12` | the binary, the SPA `dist/`, one writable dir. No shell, no package manager, no curl |

Details and the cache strategy: [references/multi-stage.md](references/multi-stage.md).

## 3. Rules

- **Pin every base by tag and digest** (`image:tag@sha256:…`), in both Containerfiles. Only
  updatecli moves them. Tools fetched in a stage are pinned by version, and checked by sha256
  where the release publishes one.
- **Lockfiles are law in the image**: `cargo build --locked`, `pnpm install --frozen-lockfile`,
  `npm install -g --ignore-scripts pnpm@<exact>`. The image must not resolve what the repo never saw.
- **Non-root, numeric**: `USER 65532:65532` in every image, the same UID the Helm chart's
  `runAsUser` pins (weebo-helm). Writable paths are pre-created in the builder and copied with
  `--chown=65532:65532`; everything else stays root-owned.
- **Runtime carries only what runs.** A role that needs extra tools (batlehub's scanner worker)
  gets its own image, never a fatter main image. See [references/hardening.md §3](references/hardening.md).
- **Health is probed from outside.** No `HEALTHCHECK`: the runtime has no shell or curl. The
  chart's liveness/readiness probes hit the server's health routes.
- **Scan and sign by digest, never by tag**: a tag is a mutable pointer.
- The two Containerfiles change together. Diff them in review.

## 4. CI

PRs that touch a Containerfile or a lockfile, pushes to main, and a daily cron build the image
with `load: true` and run the Trivy gate. The release job builds `linux/amd64,linux/arm64`,
pushes `ghcr.io/<repo>:<version>`, signs with keyless cosign, attests SBOM and provenance, and
gates again. Each image gets its own `type=gha` cache `scope`. Details:
[references/ci.md](references/ci.md). Gate severity, `.trivyignore.yaml`, VEX and SBOM policy
belong to **weebo-security**; the rest of the release workflow to **weebo-release**.

## 5. Local use

`task compose:db` starts Postgres for `cargo run`; `task compose:build` / `compose:up` build and
run the image. There is no container engine in a CDE: dependencies there are devfile sidecars
(**weebo-cde**), and images are only built in CI. Nothing in batlehub runs tests against the
built image; heavy e2e runs the binary (weebo-testing).

## 6. Known debt to avoid copying blindly

batlehub's release image step takes about 3 hours (its TD-0003). The template keeps batlehub's
shape but flags each cause; fix them when the build time matters:
arm64 under QEMU, the UI stage waiting on Rust for `dump-spec`, the server compiled once per
image. The version-bump cache miss is fixed by cargo-chef. [references/multi-stage.md §4](references/multi-stage.md).

## Templates

```
templates/
├── Containerfile                     chef → planner → builder → ui-builder → distroless runtime
├── Containerfile.hardened            same, on registry.access.redhat.com/hi/*
├── .dockerignore
├── docker-compose.yml                postgres + commented server service
├── .tasks/compose.yaml               compose:up/db/build/logs/down
├── .updatecli/manifest/docker.yaml   FROM tag+digest bumps
└── .github/workflows/
    ├── image-scan.yaml               PR / main / daily build + Trivy gate
    └── image-release.yaml            tag: build, push, sign, attest, gate
```

Replace every `my-server` / `my_server` / `my-crate` placeholder, list each top-level source
directory in the planner and builder `COPY`s, and let updatecli refresh the digests.
