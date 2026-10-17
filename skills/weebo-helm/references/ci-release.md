# CI and release

Source: batlehub@20879e5, `.github/workflows/helm-lint.yaml`, job `chart` of
`.github/workflows/build.yaml`, `.tasks/release.yaml`, `cog.toml`.

## 1. PR gate: `helm-lint.yaml`

Triggered on push/PR to `main` touching `helm/**` (path filter) and on dispatch. Two jobs:

**`lint`**: `helm dependency build`, `helm lint`, `helm template` with defaults into a file, then
grep assertions on the rendered YAML. These *are* the chart tests (no helm-unittest):

| Assertion | Catches |
| --- | --- |
| no probe `path:` under `/api/` | probe on an authenticated route (403, never Ready) |
| liveness block has `/livez`, readiness block has `/healthz` | swapped probes (greps scoped per block) |
| `runAsNonRoot: true`, `allowPrivilegeEscalation: false`, `readOnlyRootFilesystem: true` present | a values edit that drops the non-root defaults |
| render with `--set` of non-default options | template errors only those branches reach |

batlehub adds feature-specific asserts the same way (the credentials layer: both Deployments pass
the second `--config`, primary first; no checksum on the credentials Secret; `existingSecret`
replaces the rendered one; absent by default). Each new conditional feature gets a render with it
on and a grep for its silent failure mode.

**`docs`**: installs helm-docs at the commit pinned in `mise.toml` (`go install …@<sha>`),
regenerates, `git diff --exit-code` on the README.

## 2. Version

- `Chart.yaml` `version` and `appVersion` are both the release version (bare semver, no `v`).
- `task version:set VERSION=x.y.z` is cocogitto's `pre_bump_hooks` entry; among the other version
  files it runs
  `sed -i 's/^version: .*/version: '"$v"'/; s/^appVersion: .*/appVersion: "'"$v"'"/' helm/<app>/Chart.yaml`
  then `helm-docs` so the README badge moves with it. `version:check` compares
  `version`, `appVersion` and the README `AppVersion` badge with the Cargo version.
  The bump, tag and changelog belong to weebo-release.
- The release job passes the tag to `helm package --version --app-version` anyway, so the
  published chart can never disagree with the tag.

## 3. Publish: job `chart` (on tag push)

1. `azure/setup-helm` pinned `version: v3.22.0`, `sigstore/cosign-installer`, `docker/login-action`
   to `ghcr.io` with `GITHUB_TOKEN`. Permissions: `packages: write`, `id-token: write`.
2. `helm repo add` each dependency repository (Helm 4 refuses `dependency build` otherwise),
   `helm dependency build`, `helm package --destination dist --version V --app-version V`.
3. `helm push dist/<app>-V.tgz oci://ghcr.io/<owner>/<repo>/charts`, tee stderr to a log, grep the
   `sha256:` digest; **fail** if none ("refusing to sign a tag").
4. `cosign sign --yes ghcr.io/<owner>/<repo>/charts/<app>@<digest>`: keyless, identity is the
   workflow. Verify with `cosign verify --certificate-identity-regexp '^https://github.com/<repo>/'
   --certificate-oidc-issuer https://token.actions.githubusercontent.com <ref>`.

The release notes name the chart as `oci://ghcr.io/<repo>/charts`. Install:
`helm install my-server oci://ghcr.io/<repo>/charts/my-server --version V`.

All actions are pinned by commit SHA with the version in a comment (see weebo-security).
