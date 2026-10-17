# devfile.yaml

Schema: [devfile 2.3.0](https://github.com/devfile/api/blob/main/schemas/latest/devfile.json).
Template: `templates/devfile.yaml`.

## 1. Shape

| Key | Weebo use |
| --- | --- |
| `schemaVersion` | `2.3.0` |
| `metadata.name` | the project name |
| `parent.uri` | the Weebo image devfile, `…/WeeboDevImage/refs/heads/main/che-min-mise/devfile.yaml`. It brings the `tools` container (image `che-min-mise`, mise, `mountSources: true`, 8Gi limit) |
| `parent.components` | overrides of parent components, **by name**. Batlehub re-declares `tools` to raise `memoryLimit` to 15Gi and add its `front`/`back` endpoints |
| `components` | everything the project adds: sidecar containers and volumes |
| `commands`, `events` | allowed by the schema (`exec`/`apply`/`composite` commands; `postStart`, `preStop` … events). **Batlehub declares none**: workspace setup is `task cde:init`, run by hand, so it stays in the Taskfile like every other command |

A repo with nothing to add keeps only `schemaVersion`, `metadata` and `parent.uri`, which is
what this repo (weebo-skillsagent) does.

## 2. Container component

```yaml
- name: my-server                 # the name `kubectl exec -c` takes
  container:
    image: ghcr.io/org/my-server:1.2.3
    memoryLimit: 256Mi            # always set limits: every sidecar eats the pod's quota
    memoryRequest: 64Mi
    cpuLimit: "500m"
    cpuRequest: 50m
    mountSources: true            # mount /projects (shared files with tools)
    command: ["/bin/sh", "-c"]    # overrides the image ENTRYPOINT
    args: ["…"]                   # overrides the image CMD
    env:
      - name: KEY
        value: value
    volumeMounts:
      - name: my-server-data
        path: /data/
    endpoints: [...]
```

## 3. Endpoints

All containers share one network namespace, so **every port is already `localhost` from every
container**. An endpoint is only needed to name a port or to reach it from outside the pod.

| Field | Values | Rule |
| --- | --- | --- |
| `exposure` | `public` / `internal` / `none` | `public` gets a route on the workspace FQDN. Anything that can drive or read the service (CDP, webdriver) stays `internal` |
| `protocol` | `https`, `http`, `wss`, `tcp` … | `https` for a public web endpoint |
| `secure` | `true` / `false` | `true` puts Che's auth in front. Set `false` only when an unauthenticated client must fetch it (batlehub's dex: the backend fetches the OIDC discovery document, and Che would answer with a login page). Say so in a comment, and expose nothing you would mind a passer-by reaching |

From a task, batlehub gets an endpoint's public URL with `url <endpoint-name>` (a helper of the
Weebo image), and strips its trailing slash: `url back | sed 's#/$##'`. A URL that contains the
workspace name cannot be committed: render the config that needs it from a task (batlehub's
`task dex:config`).

## 4. Volumes

```yaml
- name: postgres-data
  volume:
    size: 10Gi
```

Mounted with `volumeMounts` in the container. A container's own filesystem is lost on restart;
`/projects` (`mountSources`) and named volumes survive. Point the data dir at the volume
(`PGDATA=/data/pgdata`, one level under the mount, so postgres gets an empty directory it owns).

## 5. Applying a change

Editing `devfile.yaml` changes nothing until the workspace is rebuilt from it. The user runs
**Restart Workspace from Local Devfile** (command palette in the editor) after the change is
saved. Ask the user to do it; then check the new container with
`kubectl get pod $HOSTNAME -o jsonpath='{.status.containerStatuses[*].name}'` or its task
(`task browser:check`).

A restart replaces the pod: every sidecar, its process state, shell history and anything not on
`/projects` or a volume goes with it. Batch devfile changes rather than restarting once per edit.
