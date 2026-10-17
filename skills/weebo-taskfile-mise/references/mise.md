# mise

## 1. Adding a tool

```bash
mise search <name>        # is it in the registry, and under which backend?
mise use <name>           # installs it and writes the line into mise.toml
task mise:lock:gen        # records version, URL and checksum in mise.lock
```

Ask the user before adding a tool. Commit `mise.toml` and `mise.lock` together. In a CDE there is no
root and no system package manager, so mise is the only way a tool gets onto the machine (see
`weebo-cde`).

## 2. Backends

A bare name (`task`, `trivy`, `k6`) uses mise's registry, which picks a backend for it. Use an
explicit `<backend>:<package>` key when the registry has no entry or the default backend is wrong:

| Key | Installs | Seen in |
| --- | --- | --- |
| `"cargo:cargo-audit"` | builds a crate from crates.io | both repos: cargo-audit, cargo-sweep, cargo-cyclonedx |
| `"aqua:EmbarkStudios/cargo-deny"` | a prebuilt binary through the aqua registry, faster than `cargo:` | batlehub |
| `"github:mlab-sh/postmortem"` | an asset from a GitHub release | both |
| `"github:rustfs/cli" = { version = "latest", exe = "rc" }` | same; `exe` when the binary is not named after the repo | batlehub |
| `"go:github.com/norwoodj/helm-docs/cmd/helm-docs" = "<commit>"` | `go install` at a version or commit | batlehub |
| `"pipx:graphifyy"` | a Python CLI in its own venv (through uv when it is installed) | weebo-skillsagent |

Prefer a prebuilt backend (registry, `aqua:`, `github:`) over `cargo:`/`go:`, which compile on
every machine and every upgrade.

## 3. Pinning

`"latest"` by default, per the house rule "latest version without a CVE". Pin to an exact version
when:

- **CI pins the same tool.** gitleaks is pinned in `mise.toml` and in the secret-scan workflow to
  the same version, so a local run reproduces the gate. With `latest` locally and an older default in
  the action, CI flagged a fixture the local run could not see.
- **A CVE fix is out before mise's index lists it.** batlehub pinned `go = "1.27.2"` with the CVE
  ids in a comment, and a note to go back to `latest` once `mise ls-remote go` catches up.
- **The last release is too old.** helm-docs is pinned to a commit because its last release carried
  42 toolchain CVEs; the same commit is pinned in the CI workflow.
- A major line the project depends on: `node = "26"`, `pnpm = "12"` (match `packageManager`).

Every pin carries a comment saying why and when it can go back to `latest`.

## 4. mise.lock

`mise.lock` records the resolved version, download URL and checksum per platform; `mise install`
then installs exactly that. `task mise:lock:gen` runs `mise lock --platform linux-x64`; add one
`--platform` per OS/arch that runs the project. `task mise:lock:update` runs `mise upgrade`, which
moves each tool to the newest version its pin allows and rewrites the lock. Never edit it by hand:
it starts with `# @generated`.

A CVE scan of the installed toolchain itself (`task mise:cve`, trivy `rootfs`) belongs to
`weebo-security`.

## 5. Not in mise.toml

- The nightly Rust toolchain for `cargo +nightly fuzz` (see `weebo-testing`).
- Project dependencies: those live in `Cargo.toml`, `package.json`, `pyproject.toml`.
- Machine-wide personal tools: `~/.config/mise/config.toml`, not the shared file.
