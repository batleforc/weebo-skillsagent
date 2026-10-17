---
name: weebo-helm
description: House Helm chart practice for Weebo projects, modelled on batlehub — chart layout under helm/<name>/, values.yaml conventions (non-root securityContext, resources, /livez and /healthz probes, config rendered into a Secret with a checksum rollout), helm-docs README with a drift gate, example values files under deploy/helm/, the .tasks/helm.yaml tasks, chart version tied to the app release, and the CI that lints on PR and pushes a cosign-signed chart to GHCR over OCI on tag. Use when adding a Helm chart to a project, changing a chart's values or templates, or wiring chart linting and publishing.
---

# Weebo Helm chart practice

The reference implementation is [batlehub](https://github.com/batleforc/batlehub). Every file
under `templates/` is a snapshot of batlehub at commit `20879e5`, trimmed to a minimal chart any
server can start from. When a template and this text disagree, the template wins: it is what ran.

## 1. Layout

| Path | Holds |
| --- | --- |
| `helm/my-server/` | The chart: `Chart.yaml`, `values.yaml`, `templates/`, `.helmignore`, `README.md.gotmpl` |
| `helm/my-server/README.md` | **Generated** by helm-docs. Never hand-edited |
| `helm/my-server/Chart.lock` | Committed when the chart has a dependency; `charts/` is not committed |
| `deploy/helm/values-*.yaml` | Complete `-f` files the install docs point at, one per setup (minimal, production, …) |
| `.tasks/helm.yaml` | The chart tasks, included with `flatten: true` (see weebo-taskfile-mise) |
| `.github/workflows/helm-lint.yaml` | PR gate: lint, render, assert, README drift |
| release workflow, job `chart` | Package, push to `oci://ghcr.io/<repo>/charts`, cosign sign |

Details: [references/chart.md](references/chart.md).

## 2. values.yaml conventions

| Concern | Convention |
| --- | --- |
| Docs | Every value has a `# --` comment (helm-docs). Banner comments group by concern; keep file order |
| Image | `repository` + `tag: ""` defaulting to `Chart.appVersion`. Tag, not digest (digests are signed in CI) |
| Security | `podSecurityContext` runAsNonRoot, uid/gid/fsGroup `65532` (the image's `USER`), seccomp `RuntimeDefault`; container `allowPrivilegeEscalation: false`, `readOnlyRootFilesystem: true`, drop `ALL` |
| Resources | cpu + memory + ephemeral-storage requests; memory + ephemeral-storage limits; no CPU limit |
| Probes | Hard-coded in the template, not values: liveness `/livez` (process), readiness `/healthz` (dependencies). Both unauthenticated |
| Config | `config:` object serialised with `toToml` into a **Secret**, mounted read-only, `--config` arg; `checksum/config` annotation rolls the pods |
| Secrets in config | `env` with `secretKeyRef`, referenced as `${VAR}` inside `config`; never a literal |
| ServiceAccount | Created, `automountServiceAccountToken: false` unless the app calls the API |
| Optional resources | Ingress, NetworkPolicy off by default; PDB only when `replicaCount > 1` |

## 3. Taskfile contract

| Task | Does |
| --- | --- |
| `helm:lint` | `helm dependency build` + `helm lint` |
| `helm:docs` | Regenerate the chart README with helm-docs (`--sort-values-order=file`) |
| `helm:docs:check` | Fail if the README drifted; restores the file it rewrote |
| `helm:examples` | Every `deploy/helm/values-*.yaml` renders |

Every task has a `desc`. Template: `templates/.tasks/helm.yaml`.

## 4. Tooling

Pin in `mise.toml`:

```toml
helm = "latest"
# Built from a pinned commit: the last release (v1.14.2) carries a Go 1.22
# toolchain's worth of CVEs. The same commit is pinned in helm-lint.yaml so the
# local task and the CI gate generate the same file.
"go:github.com/norwoodj/helm-docs/cmd/helm-docs" = "a5573af096a4b526dcbc3c896c220b1714a0765b"
```

CI also needs `cosign` (installed by `sigstore/cosign-installer` in the release job).

## 5. Rules

- **The chart is the non-root backstop.** CI greps the rendered defaults for `runAsNonRoot: true`,
  `allowPrivilegeEscalation: false`, `readOnlyRootFilesystem: true`. Need scratch space? Mount an
  `emptyDir`, never relax the read-only root.
- **Probes never hit an authenticated route.** The kubelet sends no credentials: a 403 means never
  Ready. Liveness on `/healthz` would restart every replica on a database blip; the CI check is
  scoped per probe block so a swap fails.
- **`toToml` turns numbers into floats.** `port = 8080.0` is refused by serde: coerce every integer
  field with `int`/`int64` in the config helper before `toToml`.
- **Test whether a value was set, not whether it is truthy.** `maxUnavailable: 0` and
  `enabled: false` are falsy in Go templates; use `hasKey` / `kindIs "invalid"`.
- **No checksum annotation on a Secret the server hot-reloads**: a rotation would become a rollout.
- **The README is generated.** Document a value by annotating it in values.yaml; prose goes in
  `README.md.gotmpl`.
- **An example values file is code.** `helm:examples` renders each one; a broken one fails in
  someone else's cluster.

## 6. Versioning and publishing

`Chart.yaml` `version` and `appVersion` both equal the release version. `task version:set`
(cocogitto pre-bump hook, see weebo-release) rewrites them and regenerates the README badge, and
`version:check` fails if they disagree with the app. On a `v*` tag the release job runs
`helm package --version X --app-version X` (tag minus `v`), `helm push` to
`oci://ghcr.io/<repo>/charts`, then `cosign sign` the pushed **digest**. Details:
[references/ci-release.md](references/ci-release.md).

## 7. Not yet practised

Batlehub does none of these, so this skill gives no convention for them:

- **helm-unittest** or **chart-testing (`ct`)**: chart tests are rendered-manifest greps in
  `helm-lint.yaml` instead.
- **Image digest pinning in values** (`image.digest`): the chart deploys by tag.
- **values.schema.json**.
- **gh-pages / classic `index.yaml` repository**: batlehub publishes over OCI only from GitHub.
- **ConfigMap for config**: config goes in a Secret because it carries credentials.

## Templates

```
templates/
├── helm/my-server/
│   ├── Chart.yaml  values.yaml  .helmignore  README.md.gotmpl
│   └── templates/  _helpers.tpl deployment.yaml secret.yaml service.yaml serviceaccount.yaml
│                   ingress.yaml pdb.yaml networkpolicy.yaml NOTES.txt
├── deploy/helm/values-minimal.yaml
├── .tasks/helm.yaml                       helm:lint, helm:docs, helm:docs:check, helm:examples
└── .github/workflows/
    ├── helm-lint.yaml                     PR gate + README drift
    └── helm-release.yaml                  the release workflow's `chart` job, standalone
```

Replace every `my-server` / `my-org` placeholder, run `task helm:docs` once to create the README,
and commit it.
