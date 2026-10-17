# Sidecars

A sidecar is a `container` component of `devfile.yaml`: one more container in the workspace pod.
It is how a CDE gets what a local machine gets from `podman compose`.

## 1. Adding one

1. Pick the image; pin a version tag when upstream publishes one (batlehub: `postgres:17-alpine`,
   `dex:v2.45.0`; its own `che-browser` image tracks `:main`).
2. Add the component (see [devfile.md §2](devfile.md)) with memory/CPU limits and requests.
3. `mountSources: true` if it must read repo files or exchange files with the tools container
   through `/projects`.
4. An endpoint only if it must be named or reached from outside the pod; `internal` unless a real
   browser needs it.
5. Add its tasks in `.tasks/<name>.yaml`, with a `<NAME>_EXEC` var (§2).
6. Ask the user to restart the workspace from the local devfile.

Batlehub's sidecars: `browser` (Chrome, CDP, noVNC), `postgres` (+ volume), `dex` (OIDC IdP,
replacing the four-container Authentik its compose file runs).

## 2. Reaching a sidecar

Its **ports** need nothing: `curl http://localhost:9222` from the tools container works.
Only a **process or file inside it** needs a hop, through `kubectl exec` against the pod itself:

```yaml
# root Taskfile.yml (global, because .tasks/ui.yaml runs gates in the sidecar too)
vars:
  BROWSER_EXEC: kubectl exec ${DEVWORKSPACE_NAMESPACE:+-n ${DEVWORKSPACE_NAMESPACE}} ${HOSTNAME} -c browser --
  BROWSER_LOGS: kubectl logs ${DEVWORKSPACE_NAMESPACE:+-n ${DEVWORKSPACE_NAMESPACE}} ${HOSTNAME} -c browser
```

- `$HOSTNAME` is the pod name; `$DEVWORKSPACE_NAMESPACE` the namespace. Che exports both into
  every container. The `:+` guard drops `-n` when the variable is unset.
- A var used by one Taskfile only stays file-local (batlehub's `DEX_EXEC` in `.tasks/dex.yaml`).
- `kubectl exec … -it` for an interactive or long-running command, plain otherwise.
- Escape what must expand **inside** the sidecar: `psql -U \$POSTGRES_USER`.
- Build a command for the sidecar with a literal `PATH` and quote it whole
  (`sh -c {{… | squote}}`): nothing should expand in the outer shell before kubectl hands the
  string over.
- `/home` and `/projects` are shared, so the sidecar can run the mise-installed `node` of the
  tools container: resolve its bin dir with
  `dirname "$(readlink -f "$(command -v node)")"` and put it on that literal `PATH`.

## 3. A sidecar that fails to start: two steps

**Trap:** in a workspace, a container that terminates does not come back on its own. The pod is
replaced, every sidecar with it (batlehub measured three lost workspaces). A sidecar whose
process crashes at boot (missing config, wrong flag) can leave the pod short of Ready and the
workspace unable to start.

So when a sidecar does not start on the first try:

1. **Step 1, in the devfile:** keep the container alive and do nothing.

   ```yaml
   - name: my-server
     container:
       image: ghcr.io/org/my-server:1.2.3
       command: ["sleep", "infinity"]
   ```

2. **Step 2, a follow-up task** that starts the real process inside it, and debug from there
   (`templates/.tasks/cde.yaml`, `cde:server:start`):

   ```yaml
   server:start:
     desc: "Start my-server inside its sidecar"
     cmds:
       - kubectl exec -c my-server -it $HOSTNAME -- my-server
   ```

Batlehub's earlier Authentik sidecars ran this way (`cde:auth:start:server` →
`kubectl exec -c authentik-server -it $HOSTNAME -- server`).

### Hardened form: wait loop + supervisor

When the process needs a rendered file and must be restartable without losing the pod, batlehub's
`dex` sidecar goes further. PID 1 is a shell that never exits; the service is its child.

```sh
cfg="${PROJECTS_ROOT:-/projects}/my-crate/dev/my-server/config.yaml"  # rendered by a task
# Without this, PID 1 sits in `wait` and shutdown waits out the grace period.
trap 'kill "${pid:-0}" 2>/dev/null; exit 0' TERM INT
# Wait instead of crash-looping, which would leave the pod short of Ready.
while [ ! -f "$cfg" ]; do echo "waiting for $cfg — run 'task my-server:config'"; sleep 5; done
while true; do
  seen="$(md5sum "$cfg")"
  my-server serve "$cfg" & pid=$!
  while kill -0 "$pid" 2>/dev/null; do
    [ "$(md5sum "$cfg")" != "$seen" ] && { kill "$pid"; break; }   # config changed: restart
    sleep 2
  done
  wait "$pid"
  # Died on an untouched config: restarting would fail the same way. Hold until it changes.
  while [ "$(md5sum "$cfg")" = "$seen" ]; do sleep 2; done
done
```

The full version (a `reload` file to force a restart, a `state` file as the ack a task waits on)
is batlehub's `devfile.yaml` `dex` component and `.tasks/dex.yaml` `dex:reload`. A reload this
way needs no kubectl and no permission the workspace lacks: the two sides talk over `/projects`.

## 4. The browser sidecar

`ghcr.io/batleforc/weebodevimage/che-browser:main`: headed Chrome for Testing behind Xvfb, with
CDP on 9222, chromedriver on 9515 and noVNC on 6080. Chromium cannot run in the tools container
(no libglib, no root); this is the replacement.

| Task | Does |
| --- | --- |
| `browser:check` | CDP answers (`curl -sf localhost:9222/json/version`); make it a `deps:` of any task that needs Chrome |
| `browser:tabs` / `browser:open URL=` / `browser:close ID=` | tabs through the CDP HTTP API |
| `browser:start` / `stop` / `restart` / `status` | `sidecar-app` inside the sidecar; `stop` parks Chrome (frees ~1 GiB), noVNC stays up |
| `browser:vnc` | prints the noVNC password from `/projects/.sidecar/vnc-password-browser.txt` |
| `browser:exec CMD=` / `browser:shell` / `browser:logs` | run, shell, follow the log |

Drive it over CDP from the tools container, or run the tool *inside* the sidecar with
`BROWSER_EXEC`, as batlehub's `ui:design:rendered` does. When run inside:

- `PUPPETEER_EXECUTABLE_PATH=/opt/chrome/chrome` reuses the sidecar's Chrome instead of
  downloading a second copy into the shared cache.
- `CI=1` (or `--no-sandbox`): unprivileged user namespaces are unavailable, and without it
  Chrome aborts with "No usable sandbox!".
- The dev server it scans is `localhost` too. Batlehub runs a second front for it
  (`ui:dev:local`, 5174, API on `localhost:8080`) beside the public one (`ui:dev`, 5173, API on
  `url back`).
