# Hardening

## 1. What both Containerfiles already do

| Measure | How |
| --- | --- |
| Small attack surface | distroless/`core-runtime`: no shell, no package manager |
| Non-root | `USER 65532:65532`, numeric so `runAsNonRoot` works without `/etc/passwd` |
| Read-only by default | binaries and SPA root-owned; only the cache dir is owned by 65532, so the chart can set `readOnlyRootFilesystem` and mount that one path |
| Reproducible inputs | bases pinned `tag@sha256:…`; `--locked`, `--frozen-lockfile`, exact pnpm version |
| No install scripts | `npm install -g --ignore-scripts` |
| Small context | `.dockerignore` |

batlehub sets no `LABEL` and no `HEALTHCHECK`. OCI metadata comes from the release (provenance
and SBOM attestations, see ci.md); health from the chart's probes. Do not add a curl/`CMD-SHELL`
healthcheck: there is nothing to run it with.

## 2. `Containerfile.hardened`

Same stages, line for line, with three swaps:

| | `Containerfile` | `Containerfile.hardened` |
| --- | --- | --- |
| builder | `rust:<ver>-slim-bookworm`, `apt-get` | `registry.access.redhat.com/hi/rust:latest-builder`, `dnf install … && dnf clean all` |
| ui-builder | `node:<ver>-slim` | `hi/nodejs:latest-builder`, preceded by `USER root` for the global pnpm install |
| runtime | `gcr.io/distroless/cc-debian12` | `hi/core-runtime` |

The UID stays 65532 so one Helm `runAsUser` fits either image. On `core-runtime` 65532 has no
passwd entry, so `$HOME` is unset: add nothing that needs a home directory. CI does not build
this file; updatecli keeps its digests current, so it compiles only when a deployer builds it.
A change to `Containerfile` that is not mirrored here goes unnoticed: review them together.

## 3. An image that must carry tools

batlehub's worker image (`Containerfile.worker`) needs bubblewrap and scanner binaries, so it
cannot be distroless. Its pattern, for any such image:

- A **separate image**, so the main image stays shell-less and does not answer for the tools' CVEs.
  An optional tool with an interpreter (GuardDog, Python) gets a third image built `FROM` the
  second via `ARG WORKER_IMAGE`; CI passes the digest just pushed so both carry the same binary.
- Tools are fetched in a throwaway `tools` stage and only the binaries are copied over.
  `curl -fsSL --proto '=https' --proto-redir '=https'`; pin the version by `ARG`, and
  `sha256sum -c` when the project publishes a digest.
- Runtime `debian:bookworm-slim@sha256:…` with **`apt-get upgrade` before `install`**: the base is
  pinned, and Debian ships security fixes to `bookworm-security` weeks before a new slim digest
  exists. Without the upgrade the daily rebuild gates on a CVE the archive already fixed.
- Create the user explicitly (`groupadd -g 65532 nonroot && useradd -u 65532 … -s /usr/sbin/nologin`)
  and `chown` the writable dirs in the same `RUN`.

## 4. Keeping bases current

`.updatecli/manifest/docker.yaml` uses updatecli's `dockerfile` autodiscovery over the files in
`filematch`, opening one PR per bump that moves tag and digest together. batlehub runs it through
`updatecli-compose.yaml` (one policy per ecosystem, shared `.updatecli/vars.yaml`) against its
Gitea mirror; adapt `scms`/`actions` to the forge. Every Containerfile must be in `filematch`;
batlehub's worker files are not, so their digests only move by hand.
