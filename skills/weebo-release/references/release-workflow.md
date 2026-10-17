# Release workflow

Two shapes, both triggered by a `v*` tag push:

| Template | From | Use when |
| --- | --- | --- |
| `templates/.github/workflows/release-cli-only.yaml` | weebo-skillsagent `release.yml` | the repo ships one CLI binary and nothing else |
| `templates/.github/workflows/release.yaml` | batlehub `build.yaml` | a server, its CLI, an image and a chart |

## 1. CLI-only

One matrix job: test, build `--locked` for the target, `tar -czf my-cli-<target>.tar.gz`, upload
with `softprops/action-gh-release`. The release is created on the first upload and published
immediately (no `draft`).

Asset names matter: mise's `github:` backend picks the archive whose name contains the OS and
arch, so `my-cli-x86_64-unknown-linux-musl.tar.gz` and `my-cli-linux-amd64.tar.gz` both work.
Users then install with:

```bash
mise use -g github:<owner>/<repo>        # latest release
```

and projects pin it in `mise.toml`, as weebo-skillsagent does for itself:
`"github:batleforc/weebo-skillsagent" = "0.10.0"`. Bumping that pin after a release is a normal
change, made by the user.

## 2. Full (batlehub)

| Job | Produces | Uploads to the draft release |
| --- | --- | --- |
| `meta` | `tag` (`v1.2.0`) and `version` (`1.2.0`) outputs | — |
| `server` | `cargo build --release --locked -p my-server`, CycloneDX SBOM, provenance + SBOM attestations; **creates the draft** with the body and `generate_release_notes: true` | binary, `sbom-rust.cdx.json` |
| `cli` | matrix: linux amd64/arm64 (musl, via `cross`), windows amd64 (zip), macOS arm64; provenance per archive | `my-cli-<os>-<arch>.tar.gz` / `.zip` |
| `image` | multi-arch image to GHCR, `cosign sign` by digest, Trivy SARIF, Syft SBOM, SBOM + provenance attestations pushed as OCI referrers, Trivy gate (`exit-code: 1` on fixable HIGH/CRITICAL) | `sbom-image.cdx.json` |
| `chart` | `helm package --version --app-version` from the tag, `helm push` to `oci://ghcr.io/<repo>/charts`, `cosign sign` by digest | — |
| `sign` | needs all of the above; downloads every asset, `sha256sum` → `checksums.txt`, `cosign sign-blob --bundle` | `checksums.txt`, `checksums.txt.sigstore.json` |

Rules it follows:

- **Draft first, published by a person.** Every upload passes `draft: true` and the same
  `tag_name`, so all jobs fill one draft. Nothing is visible until the user reviews and clicks
  Publish (or `gh release edit v1.2.0 --draft=false`).
- **Keyless signing.** `id-token: write` per job; cosign and `actions/attest-*` use the workflow's
  OIDC identity. No key in a secret.
- **Sign and attest by digest, never by tag.** A tag is mutable.
- **Least privilege.** Top-level `permissions: contents: read`; each job widens only what it needs.
- **Actions pinned by SHA** with the version in a comment.
- **The chart version comes from the tag** (`helm package --version`), so even a chart whose
  `Chart.yaml` drifted ships the right number. `version:check` still keeps the files honest.
- If the chart has dependencies from a Helm repo, `helm repo add` it before `helm dependency
  build`: Helm 4 refuses to build from a repository it was never told about.

Batlehub also builds two worker images and attaches an OpenVEX document
(`.github/scripts/vex.py render`, attested with `cosign attest --type openvex`, applied to the gate
with `TRIVY_VEX`). Add those only if the project has the same need; the scanners and VEX belong to
`weebo-security`, the Containerfiles to `weebo-containerfile`.

## 3. The perf report

Batlehub's `perf-report.yaml` runs on the same tag, separately from the release: twenty minutes
of k6 must never be the reason a release does not ship. It runs the suite with `--no-thresholds`,
downloads `perf-report.json` from the previous **published** (non-draft) release as the baseline,
and attaches `perf-report.{json,md}` to the draft. It is a record, not a gate (shared runners are
noisy). It is not covered by `checksums.txt`, by design. The workflow, the scenarios and the
report script are in `weebo-testing` (`templates/.github/workflows/perf.yaml`).

## 4. Verifying a release (what the body tells users)

```bash
cosign verify \
  --certificate-identity-regexp '^https://github.com/<owner>/<repo>/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  ghcr.io/<owner>/<repo>:1.2.0
cosign verify-blob --bundle checksums.txt.sigstore.json \
  --certificate-identity-regexp '^https://github.com/<owner>/<repo>/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  checksums.txt
sha256sum -c checksums.txt
gh attestation verify my-server --repo <owner>/<repo>
```

Run these on the draft before publishing it.
