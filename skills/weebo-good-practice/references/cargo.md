# Cargo: keep `target/` small

A CDE volume is shared and small, and most of `target/` is debug info nobody reads. These settings
cut it without touching what the security rules need: `overflow-checks` and `debug-assertions`
stay at their dev defaults [DENV-CARGO-OPTS].

## 1. Profiles (root `Cargo.toml`)

```toml
[profile.dev]
# Full DWARF is most of target/debug. Line tables keep file:line in backtraces and panics.
debug = "line-tables-only"

[profile.dev.package."*"]
# Dependencies: no debug info at all; you step into your code, not theirs.
debug = false

[profile.release]
strip = true          # no symbols in the shipped binary
lto = "thin"          # smaller, faster binary; "fat" only if the build time is acceptable
codegen-units = 1
```

`[profile.test]` inherits `dev`, so tests and coverage shrink too. Keep `panic = "unwind"` (the
default): `panic = "abort"` changes behaviour, and only the fail-fast case in ANSSI's guide
justifies it.

Need a debugger session? `CARGO_PROFILE_DEV_DEBUG=true cargo build` for that one build, not a
committed change.

## 2. Fewer things to compile

- **One version per crate.** Declare every dependency once in `[workspace.dependencies]` and use
  `dep = { workspace = true }` in members; `cargo deny` bans duplicates (`weebo-security`).
- **Only the features you use**: `default-features = false`, then list them. `cargo tree -e features
  -i <crate>` shows who turns a feature on.
- **No dependency for a few lines** (ponytail). Every crate is compiled once per profile, per target
  dir.
- **`resolver = "3"`** (edition 2024, or set it for a 2021 workspace): features of build-deps and
  dev-deps no longer leak into the normal build.

## 3. Fewer copies of everything

Each of these is a full second `target/`:

| Source | Lives in | Contain it |
| --- | --- | --- |
| `cargo llvm-cov` | `target/llvm-cov-target/` | `cargo llvm-cov clean --workspace` after a coverage run |
| `cargo fuzz` | `fuzz/target/` | its own workspace; `cargo clean` inside `fuzz/` when done |
| `--release` builds | `target/release/` | only in CI and the Containerfile (deps cached with cargo-chef, weebo-containerfile), not for local checks |
| rust-analyzer with a different `--target` or features | `target/` again | keep its settings equal to `task check` |

## 4. Cleaning

Add a task, so cleaning is one command and not a guess:

```yaml
  clean:stale:
    desc: Drop build artefacts unused for 7 days from target/ (keeps what you build daily)
    cmds:
      - cargo sweep --time 7
```

with `"cargo:cargo-sweep" = "latest"` in `mise.toml`. When `target/` still passes a few GB,
`cargo clean` (the house rule: big `target/` gets deleted).
