# SBOM and VEX

Source: batlehub@20879e5 (`.tasks/security.yaml`, `.github/workflows/build.yaml`,
`.github/scripts/vex.py`, `vex/batlehub.openvex.json`).

## SBOMs: match a future CVE against a build already shipped

| SBOM | Made by | When |
| --- | --- | --- |
| `sbom-rust.cdx.json`: the shipped server's crate closure | `cargo cyclonedx --format json` (`task sbom:rust`) | locally in `task security`, and at release |
| `sbom-image.cdx.json`: one per image (OS packages + binaries) | syft via `anchore/sbom-action`, `format: cyclonedx-json`, on the pushed **digest** | at release |

`cargo cyclonedx` writes one `<crate>.cdx.json` per workspace member; the task keeps the server's
as `sbom-rust.cdx.json` and deletes the rest. Every image gets one, the fat ones first: an image that
carries a second toolchain is exactly the one whose contents nobody can derive from the lockfiles.

Months later, no rebuild needed:

```bash
trivy sbom sbom-image.cdx.json
trivy sbom sbom-rust.cdx.json
grype sbom:sbom-image.cdx.json
```

Attaching the SBOMs to the release, binding them to the digest (`actions/attest-sbom`,
`push-to-registry: true`), provenance and cosign signing are release steps: see `weebo-release`.

## VEX: what the standing findings mean

`.trivyignore.yaml` quiets the gate, but only Trivy reads it and it never leaves the repo. A
consumer scanning the published image sees the same finding and no sign that it was assessed.
`vex/my-project.openvex.json` states the same decisions in OpenVEX, which every mainstream scanner
reads.

Add it when `.trivyignore.yaml` gets its first entry (`vex.py check` fails on a document with no
statements). Then add `task vex` to `task security` and enable the `vex` job in `image-scan.yaml`.

`task vex` (`vex.py check`) enforces:

- every statement has a `timestamp`;
- `not_affected` carries one of OpenVEX's five justifications **and** an `impact_statement` in
  prose (the justification is for the scanner, the prose for the person who has to believe it);
- `affected` carries an `action_statement`;
- every `.trivyignore.yaml` id has a statement, and every `not_affected` statement is still
  ignored by the gate. Drift fails in both directions.

The expiry stays in `.trivyignore.yaml` only.

At release, `task vex:render VERSION=1.2.3 IMAGES='ghcr.io/o/my-server@sha256:…'` rewrites each
product from a repository name to the **digest** actually published (a tag is refused) and drops
statements for images the release did not publish. Consumers apply it with
`trivy image --vex my-project.openvex.json <image>`.

For a CVE in our own code, the statement is `fixed` for the release with the fix and `affected` with
an "upgrade to" `action_statement` for the one before; `vex.py` ties neither to `.trivyignore.yaml`.
