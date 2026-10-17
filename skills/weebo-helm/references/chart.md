# The chart

Source: batlehub@20879e5, `helm/batlehub/`. The template in `templates/helm/my-server/` keeps the
generic part; this page lists what batlehub adds and why, so you know when to copy it.

## 1. Files

| File | Notes |
| --- | --- |
| `Chart.yaml` | `apiVersion: v2`, `type: application`, `version` = `appVersion` = release. `home`, `sources`, `maintainers`, `keywords` |
| `values.yaml` | Grouped by concern with `# ── Section ──` banners; every key has a `# --` helm-docs comment, `# @default --` when the literal default is unhelpful (`Chart.appVersion`, `disabled`) |
| `README.md.gotmpl` | Hand-written prose (install, what to change before exposing it) around helm-docs `{{ template "chart.*" }}` calls |
| `.helmignore` | Excludes `README.md.gotmpl` from the packaged chart |
| `templates/_helpers.tpl` | name / fullname / labels / selectorLabels / serviceAccountName, plus the config renderer |
| `templates/NOTES.txt` | How to reach the service, and a reminder of the placeholder secrets to change |

## 2. Config as a Secret

- `values.config` mirrors the server's TOML config one to one (snake_case keys). The helper
  `deepCopy`s it, coerces integer fields (`int` / `int64`, one line per known field: Helm holds
  every YAML number as float64), then `toToml`.
- An optional section must be **absent**, not `null` or `{}`, or it renders as an empty table.
- Rendered into `<fullname>-config` Secret, key `config.toml`, mounted read-only at
  `/etc/<app>/`, passed as `--config`.
- `checksum/config: {{ include "<app>.config" . | sha256sum }}` on the pod template, so a config
  change rolls the Deployment.

What batlehub adds on top, only when the app supports it:

- **Credentials layer** (`credentials.enabled`): a second `--config` file merged over the first,
  from a chart-rendered Secret or `credentials.existingSecret`. Mounted as its own volume, passed
  after the primary (order decides the merge), and **no checksum annotation**: the server
  hot-reloads it, and a rollout on rotation is what the layer exists to avoid.
- **`externalManifest`**: extra Secret/ConfigMap/any manifests rendered with `tpl` or merely
  referenced (External Secrets, Sealed Secrets), optionally mounted; one may replace the config
  Secret (`mount.asConfig`). Invalid combinations `fail` at template time.
- **`env` / `envFrom`** for `${VAR}` placeholders the server expands at load; a missing variable
  is a startup error naming it.

## 3. Security and resources

- Pod: `runAsNonRoot`, `runAsUser/runAsGroup/fsGroup: 65532`, `seccompProfile: RuntimeDefault`.
  This matches the image's `USER 65532` (weebo-containerfile) so a PSA `restricted` namespace admits
  it; `fsGroup` makes a PVC writable.
- Container: `allowPrivilegeEscalation: false`, `readOnlyRootFilesystem: true`, `drop: [ALL]`.
- Writable paths are volumes: a PVC for a cache, or an `emptyDir` with a `sizeLimit` below the
  container's `ephemeral-storage` limit (otherwise the node evicts the pod).
- Resources: requests `cpu 100m / memory 128Mi / ephemeral-storage 256Mi`, limits
  `memory 512Mi / ephemeral-storage 1Gi`. No CPU limit.
- ServiceAccount `automountServiceAccountToken: false`; batlehub forces it `true` (and renders a
  TokenReview ClusterRole) only when Kubernetes auth is configured.

## 4. Probes

Written in the Deployment template, not exposed in values:

| Probe | Path | Delay / period | Why |
| --- | --- | --- | --- |
| liveness | `/livez` | 10s / 30s | process only; restarting cannot fix a dependency |
| readiness | `/healthz` | 5s / 10s | 503 when the DB/object store is down, so the pod leaves the endpoints |

Both routes are exempt from auth in the server; any other path answers 403 to the kubelet.

## 5. Optional resources

| Resource | Rendered when | Trap |
| --- | --- | --- |
| Ingress | `ingress.enabled` | `extraHosts` adds rules for the same backend; the TLS cert needs a SAN for each, wildcard included |
| PodDisruptionBudget | `enabled` and `replicaCount > 1` | A PDB over one replica blocks drains. `maxUnavailable` wins when *set* (incl. `0`): test with `kindIs "invalid"`, not truthiness |
| NetworkPolicy | `networkPolicy.enabled` | Off by default. DNS egress always first (Egress makes it default-deny). Empty `ingressFrom` means **any source** |
| PVC | filesystem storage with `persistence.enabled` | `ReadWriteOnce` is effectively single-replica; multi-replica needs S3 or RWX |
| Sub-chart | `condition: <name>.enabled` in `Chart.yaml` | `Chart.lock` committed, `charts/` not; `helm repo add` before `dependency build` (Helm 4) |

batlehub also ships a second Deployment (`worker`, same binary, own image and securityContext) and
an HPA on an external metric; copy that shape only for a real second role.

## 6. Example values files

`deploy/helm/values-{minimal,production,…}.yaml`: each a complete, commented `-f` file named in
the install docs. `task helm:examples` renders each one. batlehub then pulls `config.toml` out of
the rendered Secret and loads it with the server's `explain-config` subcommand, failing on errors
and on `!!` warnings, and `helm:examples:check` / `:sync` (a Python script) keep the inline YAML
in `docs/guide/install/helm.md` identical to the files. Add both once the docs inline the files and
the server can validate a config.
