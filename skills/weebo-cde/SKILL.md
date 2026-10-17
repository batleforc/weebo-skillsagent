---
name: weebo-cde
description: Working inside a Weebo Eclipse Che cloud development environment (active when the WORKSPACE_NAME env var is non-empty) — no root and no container engine, tools via mise, services as devfile.yaml sidecars, reaching a sidecar with kubectl exec on $HOSTNAME, the browser sidecar for headed Chrome/CDP, the sleep-infinity fallback for a sidecar that will not start, code-oss, disk usage, and restarting from the local devfile. Use when WORKSPACE_NAME is set and you need a tool, a database or other service, a browser, or a devfile change, or when editing devfile.yaml or the cde/browser tasks of a project.
---

# Weebo cloud development environment

The reference implementation is [batlehub](https://github.com/batleforc/batlehub). Every file
under `templates/` is a snapshot of batlehub at commit `20879e5`, trimmed. When a template and
this text disagree, the template wins: it is what ran.

## 1. Am I in one?

`[ -n "$WORKSPACE_NAME" ]`. If it is empty, none of this applies: you are on a machine with
podman, and batlehub's `compose:*` tasks (`podman compose up -d` …) are the way to run services.

## 2. Rules

| Need | In a CDE |
| --- | --- |
| A CLI tool | `mise search <tool>`, then **ask the user**, then `mise use <tool>`. No root: `apt`/`dnf`/`sudo` never work |
| A service (database, IdP, browser …) | a **sidecar** in `devfile.yaml`. There is no docker/podman in the workspace, so `compose:*` tasks do not run here |
| The pod name | `$HOSTNAME` (or `hostname`); the namespace is `$DEVWORKSPACE_NAMESPACE` |
| A process or file inside a sidecar | `kubectl exec ${DEVWORKSPACE_NAMESPACE:+-n ${DEVWORKSPACE_NAMESPACE}} ${HOSTNAME} -c <container> -- <cmd>` |
| A port of a sidecar | plain `localhost:<port>`: the pod is one network namespace |
| The public URL of an endpoint | `url <endpoint-name>` (Weebo image helper; strip the trailing `/`) |
| The `code` CLI | `code-oss` when `code` is absent (`code-oss --add <dir>`) |
| A devfile change to take effect | the user runs **Restart Workspace from Local Devfile**. Ask; you cannot |
| A sidecar that will not start | two steps: `command: ["sleep", "infinity"]`, then a task that `kubectl exec`s the real process |
| Disk | watch `target/` and `node_modules/`: `du -sh target ui/node_modules`. Delete them when they grow; `cargo sweep --time 1` trims stale cargo artefacts |

## 3. Taskfile wiring

```yaml
includes:
  cde: .tasks/cde.yaml                                          # cde:init, cde:db:init, cde:*:start
  browser: { taskfile: .tasks/browser.yaml, flatten: true }     # browser:*

vars:
  # Global rather than file-local when several .tasks files run things in the sidecar.
  BROWSER_EXEC: kubectl exec ${DEVWORKSPACE_NAMESPACE:+-n ${DEVWORKSPACE_NAMESPACE}} ${HOSTNAME} -c browser --
  BROWSER_LOGS: kubectl logs ${DEVWORKSPACE_NAMESPACE:+-n ${DEVWORKSPACE_NAMESPACE}} ${HOSTNAME} -c browser
```

One `<NAME>_EXEC` / `<NAME>_LOGS` pair per sidecar you need to reach. The rest of the Taskfile
layout is `weebo-taskfile-mise`. A fresh workspace runs `task cde:init` once (extra databases …).

## 4. References

- [references/devfile.md](references/devfile.md): devfile shape, the parent Weebo image,
  overriding `tools`, endpoints (`exposure`, `secure`), volumes, commands/events, restarting.
- [references/sidecars.md](references/sidecars.md): adding a sidecar, the `kubectl exec` hop,
  the two-step start and the supervisor pattern, the browser sidecar (CDP, noVNC, running a
  scanner inside it).

## 5. Traps

- **A container that exits is not restarted; the pod is replaced**, and every sidecar with it.
  A sidecar must never crash-loop at boot: wait for its config, or `sleep infinity`.
- **A restart wipes everything outside `/projects` and named volumes.** Keep data on a volume,
  shared files on `/projects`.
- **CDP and webdriver stay `exposure: internal`.** Anything that reaches them drives the browser.
- **A workspace URL cannot be committed** (it contains the workspace name): render the file that
  needs it from a task.
- **Escape what must expand inside the sidecar** (`\$POSTGRES_USER`), and quote the whole
  `sh -c` string.

## Templates

```
templates/
├── devfile.yaml          parent Weebo image + tools endpoints, browser sidecar, postgres + volume
├── .tasks/cde.yaml       cde:init, cde:db:init, cde:server:start (second step of sleep infinity)
└── .tasks/browser.yaml   browser:check/tabs/open/start/stop/vnc/exec/shell/logs
```

Replace every `my-crate` / `my_crate` / `my-server` placeholder. Related skills:
`weebo-taskfile-mise` (Taskfile, mise), `weebo-testing` (what runs against these services),
`weebo-containerfile` (the image the project ships, not the workspace).
