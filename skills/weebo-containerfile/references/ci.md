# Building images in CI

## 1. Per-PR, main and daily: `image-scan.yaml`

Triggers on the Containerfile, `Cargo.lock`, `ui/pnpm-lock.yaml`, the ignore file and itself,
plus a daily cron: a CVE published against a base layer after the last build must still trip
CI. Steps: buildx → `build-push-action` with `load: true, push: false` → Trivy SARIF (uploaded,
never fails) → Trivy table with `exit-code: "1"` (the gate).

Chaining two images in one job (the GuardDog image `FROM` the worker): a buildx
`docker-container` builder cannot read the runner's image store, so batlehub runs a
`registry:3` service on `localhost:5000`, sets `driver-opts: network=host`, pushes the first
image there, and passes it as a build arg to the second.

## 2. On a tag: the `image` job

Template: `templates/.github/workflows/image-release.yaml` (in batlehub, the `meta` and `image`
jobs of `build.yaml`, next to binaries, chart and checksums; see weebo-release).

1. `setup-qemu` + `setup-buildx`, `cosign-installer`, `docker/login-action` to GHCR with
   `GITHUB_TOKEN` (`packages: write`).
2. `build-push-action`: `platforms: linux/amd64,linux/arm64`, one tag `:<version>` (the git tag
   without its `v`), `provenance: false`, `cache-from/to: type=gha,…,scope=<image>`. No `:latest`
   tag: the docs tell users to pin a version.
3. `cosign sign --yes <image>@<digest>`: keyless, `id-token: write`.
4. Trivy SARIF on the pushed digest.
5. Syft SBOM (CycloneDX) → `attest-sbom` and `attest-build-provenance`, both
   `push-to-registry: true` so they travel as OCI referrers of the digest.
6. Trivy gate on the digest.

batlehub also renders and attests an OpenVEX document and passes it to the final gate as
`TRIVY_VEX`; that and the SBOM uploads to the release page are weebo-security / weebo-release.

## 3. Rules

- Every action pinned by full commit SHA with the version in a comment.
- Reference the pushed image by `steps.<id>.outputs.digest` everywhere after the build.
- One cache `scope` per image, on both `cache-from` and `cache-to`.
- An image built only for amd64 says why (batlehub's worker fetches an x86_64 release asset).
