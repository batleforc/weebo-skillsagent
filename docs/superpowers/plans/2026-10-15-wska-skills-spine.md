# wska Skills Spine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `wska`, a Rust CLI that links a named subset of this repo's skills into any project's or the user's Claude Code configuration, and vendors third-party skills with upstream-drift detection.

**Architecture:** Skills are plain directories in `skills/`; installing one means creating a symlink in a target's `.claude/skills/`. A profile names a set of items; `install` is declarative — the named profiles define the complete set for a scope, and links the CLI owns but no longer wants are removed. Ownership is derived from each symlink's target resolving inside this repository, so there is no lockfile. Everything is planned and validated before any filesystem write.

**Tech Stack:** Rust 2021, clap (derive), serde + toml (read), toml_edit (write-back preserving comments), sha2 + hex, thiserror, tempfile (dev), `git` via `std::process::Command`. Task + mise for wiring.

**Spec:** `docs/superpowers/specs/2026-10-15-skills-spine-design.md`

## Global Constraints

- Crate and binary name: `wska`. Workspace member at `crates/wska/`.
- Item names must be unique across `skills/`, `agents/`, `mcp/` and `[installers]`.
- Scope is never stored in `profiles.toml`; it is always a CLI argument.
- No lockfile. Ownership is derived: a target entry is owned when it is a symlink whose target resolves inside the repository root.
- Nothing touches the filesystem until the whole plan validates. A validation failure changes nothing.
- `install` run twice with identical arguments makes zero changes and exits 0.
- An installer whose declared `scope` differs from the invocation's scope is skipped with a notice, never an error.
- Installer items are never auto-removed.
- Directory hash: SHA-256 over entries sorted by repo-relative path; each entry contributes its path bytes, `0x00`, a kind byte (`b'f'` file, `b'l'` symlink), the content length as u64 little-endian, then the contents (for a symlink, its target text). Modes, timestamps and ownership are excluded. **This adds a kind byte to the spec's definition** so that a file containing `../y` cannot hash identically to a symlink pointing at `../y`; flag it to the author when the first task lands.
- Reserved profile name `all` means every item in the repo.
- Commits: per `CLAUDE.md`, this project forbids an agent committing directly. Every "commit" step writes the message to a file and prints the `rtk git` command for the author to run. Do not run `git commit`.
- Commit messages follow Conventional Commits (`cog verify` runs in the commit-msg hook) and end with `Co-Authored-By:` and `Claude-Session:` trailers, which is how co-ownership is recorded per `CLAUDE.md`.
- All shell commands are prefixed `rtk` per `CLAUDE.md`.

## Review Focus

1. **Relative symlink targets.** A link stored as `../../skills/ponytail` resolves against the link's parent directory, not the process working directory. Classifying it from the wrong base marks the CLI's own links as foreign and refuses to install. Owned by Task 7.
2. **The target directory is itself a symlink.** `~/.claude` is commonly a link into a dotfiles repo. Both the target directory and the repository root must be canonicalized before comparison, or every entry classifies as foreign. Owned by Task 6.
3. **Re-running after an interrupted apply.** A partially applied plan must converge on the next run, not error on the links that already exist. Owned by Task 9.
4. **A user-defined profile named `all`.** The reserved name must win and say so, rather than silently shadowing the author's profile or being shadowed by it. Owned by Task 5.
5. **A symlink inside a vendored skill.** Hashing must record the link's target text and never follow it; following can escape the skill directory or loop forever. Owned by Task 2.

---

### Task 1: Workspace and CLI argument parsing

**Files:**
- Create: `Cargo.toml`
- Create: `crates/wska/Cargo.toml`
- Create: `crates/wska/src/main.rs`
- Create: `crates/wska/src/cli.rs`
- Create: `rust-toolchain.toml`

**Interfaces:**
- Consumes: nothing.
- Produces: `cli::Cli`, `cli::Command`, `cli::Scope`. `Cli::parse()` from clap. `Scope` is `Project | Project`-style enum with `#[serde(rename_all = "lowercase")]` so later tasks can deserialize it from `vendor.toml`.

- [ ] **Step 1: Create the workspace manifest**

`Cargo.toml`:

```toml
[workspace]
members = ["crates/wska"]
resolver = "2"

[workspace.package]
edition = "2021"
license = "Apache-2.0"

[workspace.dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
toml_edit = "0.22"
sha2 = "0.10"
hex = "0.4"
thiserror = "2"
tempfile = "3"
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

`crates/wska/Cargo.toml`:

```toml
[package]
name = "wska"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
clap.workspace = true
serde.workspace = true
toml.workspace = true
toml_edit.workspace = true
sha2.workspace = true
hex.workspace = true
thiserror.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Write the failing test**

`crates/wska/src/cli.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_install_with_two_profiles_and_scope() {
        let cli = Cli::parse_from([
            "wska", "install", "--profile", "base", "--profile", "frontend", "--scope", "user",
        ]);
        match cli.command {
            Command::Install { profile, scope, dry_run } => {
                assert_eq!(profile, vec!["base".to_string(), "frontend".to_string()]);
                assert_eq!(scope, Scope::User);
                assert!(!dry_run);
            }
            other => panic!("expected Install, got {other:?}"),
        }
    }

    #[test]
    fn scope_defaults_to_project() {
        let cli = Cli::parse_from(["wska", "status"]);
        match cli.command {
            Command::Status { scope } => assert_eq!(scope, Scope::Project),
            other => panic!("expected Status, got {other:?}"),
        }
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `rtk cargo test -p wska`
Expected: FAIL — `cannot find type Cli in this scope`.

- [ ] **Step 4: Write the minimal implementation**

Prepend to `crates/wska/src/cli.rs`:

```rust
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "wska", about = "Link this repo's skills into a Claude Code target")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Link the items named by the given profiles, removing owned links not in that set
    Install {
        #[arg(long, required = true)]
        profile: Vec<String>,
        #[arg(long, value_enum, default_value_t = Scope::Project)]
        scope: Scope,
        #[arg(long)]
        dry_run: bool,
    },
    /// Report what is linked in a target
    Status {
        #[arg(long, value_enum, default_value_t = Scope::Project)]
        scope: Scope,
    },
    /// List items in this repo, or profiles with --profiles
    List {
        #[arg(long)]
        profiles: bool,
    },
    /// Refresh vendored sources from upstream
    Sync {
        source: Vec<String>,
        #[arg(long)]
        check: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Project,
    User,
}
```

`crates/wska/src/main.rs`:

```rust
mod cli;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    eprintln!("not implemented: {:?}", cli.command);
    std::process::exit(1);
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `rtk cargo test -p wska`
Expected: PASS, 2 tests.

- [ ] **Step 6: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat(wska): add workspace and CLI argument parsing

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add Cargo.toml rust-toolchain.toml crates/ && rtk git commit -s -F /tmp/wska-commit.txt'
```

Hand that command to the author. Do not run `git commit` yourself.

---

### Task 2: Directory hashing

**Files:**
- Create: `crates/wska/src/hash.rs`
- Modify: `crates/wska/src/main.rs` (add `mod hash;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `hash::hash_dir(root: &Path) -> std::io::Result<String>`, returning `"sha256:<64 hex chars>"`.

- [ ] **Step 1: Write the failing tests**

`crates/wska/src/hash.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn write(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    #[test]
    fn stable_across_reruns() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "SKILL.md", "hello");
        write(d.path(), "references/a.md", "a");
        assert_eq!(hash_dir(d.path()).unwrap(), hash_dir(d.path()).unwrap());
    }

    #[test]
    fn content_change_changes_hash() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "SKILL.md", "hello");
        let before = hash_dir(d.path()).unwrap();
        write(d.path(), "SKILL.md", "hello!");
        assert_ne!(before, hash_dir(d.path()).unwrap());
    }

    #[test]
    fn mtime_change_does_not_change_hash() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "SKILL.md", "hello");
        let before = hash_dir(d.path()).unwrap();
        let p = d.path().join("SKILL.md");
        let body = fs::read(&p).unwrap();
        fs::remove_file(&p).unwrap();
        fs::write(&p, body).unwrap();
        assert_eq!(before, hash_dir(d.path()).unwrap());
    }

    #[test]
    fn two_dirs_with_same_content_hash_equal() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        for d in [a.path(), b.path()] {
            write(d, "z.md", "z");
            write(d, "a.md", "a");
        }
        assert_eq!(hash_dir(a.path()).unwrap(), hash_dir(b.path()).unwrap());
    }

    // Review Focus 5
    #[test]
    fn symlink_is_hashed_as_target_text_and_never_followed() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "SKILL.md", "hello");
        symlink("../../../etc/passwd", d.path().join("escape")).unwrap();
        // Must not error and must not read the escaped file.
        let h = hash_dir(d.path()).unwrap();
        assert!(h.starts_with("sha256:"));

        // A file whose contents equal the link text must hash differently.
        let e = tempfile::tempdir().unwrap();
        write(e.path(), "SKILL.md", "hello");
        write(e.path(), "escape", "../../../etc/passwd");
        assert_ne!(h, hash_dir(e.path()).unwrap());
    }

    #[test]
    fn symlink_cycle_terminates() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "SKILL.md", "hello");
        fs::create_dir(d.path().join("sub")).unwrap();
        symlink(d.path(), d.path().join("sub/loop")).unwrap();
        assert!(hash_dir(d.path()).is_ok());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p wska hash::`
Expected: FAIL — `cannot find function hash_dir`.

- [ ] **Step 3: Write the minimal implementation**

Prepend to `crates/wska/src/hash.rs`:

```rust
use sha2::{Digest, Sha256};
use std::io;
use std::path::{Path, PathBuf};

/// SHA-256 over directory entries sorted by relative path. Each entry
/// contributes: relative path bytes, 0x00, a kind byte (b'f' file, b'l'
/// symlink), content length as u64 LE, contents. For a symlink the contents
/// are the link's target text; links are never followed. Modes, timestamps and
/// ownership are excluded.
pub fn hash_dir(root: &Path) -> io::Result<String> {
    let mut entries: Vec<(Vec<u8>, u8, Vec<u8>)> = Vec::new();
    collect(root, Path::new(""), &mut entries)?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut h = Sha256::new();
    for (rel, kind, body) in entries {
        h.update(&rel);
        h.update([0u8]);
        h.update([kind]);
        h.update((body.len() as u64).to_le_bytes());
        h.update(&body);
    }
    Ok(format!("sha256:{}", hex::encode(h.finalize())))
}

fn collect(
    dir: &Path,
    rel_dir: &Path,
    out: &mut Vec<(Vec<u8>, u8, Vec<u8>)>,
) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let rel: PathBuf = rel_dir.join(&name);
        // symlink_metadata never follows, so a link to a directory is recorded
        // as a link and not descended into. That is what terminates cycles.
        let md = std::fs::symlink_metadata(entry.path())?;
        if md.file_type().is_symlink() {
            let target = std::fs::read_link(entry.path())?;
            out.push((
                rel.as_os_str().as_encoded_bytes().to_vec(),
                b'l',
                target.as_os_str().as_encoded_bytes().to_vec(),
            ));
        } else if md.is_dir() {
            collect(&entry.path(), &rel, out)?;
        } else {
            out.push((
                rel.as_os_str().as_encoded_bytes().to_vec(),
                b'f',
                std::fs::read(entry.path())?,
            ));
        }
    }
    Ok(())
}
```

Add `mod hash;` to `crates/wska/src/main.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p wska hash::`
Expected: PASS, 6 tests.

- [ ] **Step 5: Tell the author about the spec clarification**

The kind byte is an addition to the spec's hash definition (Global Constraints explains why). Say so in the task's report so the author can accept it into the spec.

- [ ] **Step 6: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat(wska): hash vendored directories for drift detection

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add crates/wska/src/hash.rs crates/wska/src/main.rs && rtk git commit -s -F /tmp/wska-commit.txt'
```

---

### Task 3: Config parsing

**Files:**
- Create: `crates/wska/src/config.rs`
- Modify: `crates/wska/src/main.rs` (add `mod config;`)

**Interfaces:**
- Consumes: `cli::Scope`.
- Produces:
  - `config::Profiles { pub profiles: BTreeMap<String, Profile> }`
  - `config::Profile { pub include: Vec<String>, pub items: Vec<String> }`
  - `config::Vendor { pub sources: BTreeMap<String, Source>, pub installers: BTreeMap<String, Installer> }`
  - `config::Source { pub upstream: String, pub git_ref: String, pub subpath: String, pub provides: Provides, pub licence: Option<String>, pub commit: Option<String>, pub hashes: BTreeMap<String, String> }`
  - `config::Provides` with `All` and `Named(Vec<String>)`
  - `config::Installer { pub command: String, pub scope: Scope }`
  - `config::load_profiles(path: &Path) -> Result<Profiles, ConfigError>`
  - `config::load_vendor(path: &Path) -> Result<Vendor, ConfigError>`

- [ ] **Step 1: Write the failing tests**

`crates/wska/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("c.toml");
        std::fs::write(&p, body).unwrap();
        (d, p)
    }

    #[test]
    fn parses_profiles_with_include_and_defaults() {
        let (_d, p) = tmp(
            r#"
[profiles.base]
items = ["ponytail"]

[profiles.frontend]
include = ["base"]
items = ["impeccable"]
"#,
        );
        let got = load_profiles(&p).unwrap();
        assert_eq!(got.profiles["base"].include, Vec::<String>::new());
        assert_eq!(got.profiles["frontend"].include, vec!["base".to_string()]);
        assert_eq!(got.profiles["frontend"].items, vec!["impeccable".to_string()]);
    }

    #[test]
    fn parses_vendor_source_with_star_provides() {
        let (_d, p) = tmp(
            r#"
[sources.superpowers]
upstream = "https://example.invalid/x"
ref = "main"
subpath = "skills"
provides = "*"
licence = "MIT"
commit = "abc1234"
hashes = { brainstorming = "sha256:00" }

[installers.bmad]
command = "npx bmad-method install"
scope = "project"
"#,
        );
        let got = load_vendor(&p).unwrap();
        let s = &got.sources["superpowers"];
        assert_eq!(s.git_ref, "main");
        assert!(matches!(s.provides, Provides::All));
        assert_eq!(s.hashes["brainstorming"], "sha256:00");
        assert_eq!(got.installers["bmad"].scope, crate::cli::Scope::Project);
    }

    #[test]
    fn parses_vendor_source_with_named_provides() {
        let (_d, p) = tmp(
            r#"
[sources.two]
upstream = "https://example.invalid/x"
ref = "v1"
subpath = "skills"
provides = ["a", "b"]
licence = "MIT"
"#,
        );
        let got = load_vendor(&p).unwrap();
        match &got.sources["two"].provides {
            Provides::Named(v) => assert_eq!(v, &vec!["a".to_string(), "b".to_string()]),
            other => panic!("expected Named, got {other:?}"),
        }
    }

    #[test]
    fn rejects_provides_string_that_is_not_star() {
        let (_d, p) = tmp(
            r#"
[sources.bad]
upstream = "https://example.invalid/x"
ref = "main"
subpath = "skills"
provides = "everything"
licence = "MIT"
"#,
        );
        let err = load_vendor(&p).unwrap_err().to_string();
        assert!(err.contains("provides"), "unexpected error: {err}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p wska config::`
Expected: FAIL — `cannot find function load_profiles`.

- [ ] **Step 3: Write the minimal implementation**

Prepend to `crates/wska/src/config.rs`:

```rust
use crate::cli::Scope;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("reading {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("parsing {path}: {source}")]
    Parse { path: PathBuf, source: toml::de::Error },
    #[error("source `{source_name}`: provides must be \"*\" or a list of names, got \"{got}\"")]
    Provides { source_name: String, got: String },
}

#[derive(Debug, Default, Deserialize)]
pub struct Profiles {
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub items: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Vendor {
    #[serde(default)]
    pub sources: BTreeMap<String, Source>,
    #[serde(default)]
    pub installers: BTreeMap<String, Installer>,
}

#[derive(Debug, Deserialize)]
pub struct Source {
    pub upstream: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub subpath: String,
    pub provides: Provides,
    pub licence: Option<String>,
    pub commit: Option<String>,
    #[serde(default)]
    pub hashes: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Provides {
    Star(String),
    Named(Vec<String>),
}

#[derive(Debug, Deserialize)]
pub struct Installer {
    pub command: String,
    pub scope: Scope,
}

pub fn load_profiles(path: &Path) -> Result<Profiles, ConfigError> {
    parse(path)
}

pub fn load_vendor(path: &Path) -> Result<Vendor, ConfigError> {
    let vendor: Vendor = parse(path)?;
    for (name, source) in &vendor.sources {
        if let Provides::Star(s) = &source.provides {
            if s != "*" {
                return Err(ConfigError::Provides {
                    source_name: name.clone(),
                    got: s.clone(),
                });
            }
        }
    }
    Ok(vendor)
}

fn parse<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ConfigError> {
    let body = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&body).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}
```

Rename the `Provides::All` reference in the Step 1 test to `Provides::Star(_)`:

```rust
assert!(matches!(s.provides, Provides::Star(ref s) if s == "*"));
```

Add `mod config;` to `crates/wska/src/main.rs`. Derive `Debug` is already present on `Provides`, which the tests' `{other:?}` needs.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p wska config::`
Expected: PASS, 4 tests.

- [ ] **Step 5: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat(wska): parse profiles.toml and vendor.toml

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add crates/wska/src/config.rs crates/wska/src/main.rs && rtk git commit -s -F /tmp/wska-commit.txt'
```

---

### Task 4: Repo item scan

**Files:**
- Create: `crates/wska/src/item.rs`
- Modify: `crates/wska/src/main.rs` (add `mod item;`)

**Interfaces:**
- Consumes: `config::Vendor`, `cli::Scope`.
- Produces:
  - `item::Kind { Skill, Agent, Mcp }` with `Kind::subdir(self) -> &'static str` returning `"skills" | "agents" | "mcp"`.
  - `item::Item` enum:
    ```rust
    pub enum Item {
        Linked { name: String, kind: Kind, path: PathBuf },
        Installer { name: String, command: String, scope: Scope },
    }
    ```
    with `Item::name(&self) -> &str`.
  - `item::scan(repo_root: &Path, vendor: &Vendor) -> Result<BTreeMap<String, Item>, ScanError>`
  - `item::ScanError::DuplicateName { name, first, second }`

- [ ] **Step 1: Write the failing tests**

`crates/wska/src/item.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Installer, Vendor};
    use std::fs;

    fn repo(dirs: &[(&str, &str)]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (kind_dir, name) in dirs {
            let p = d.path().join(kind_dir).join(name);
            fs::create_dir_all(&p).unwrap();
            fs::write(p.join("SKILL.md"), "x").unwrap();
        }
        d
    }

    fn vendor_with_installer(name: &str, scope: Scope) -> Vendor {
        let mut v = Vendor::default();
        v.installers.insert(
            name.to_string(),
            Installer { command: "echo hi".into(), scope },
        );
        v
    }

    #[test]
    fn finds_skills_and_agents() {
        let d = repo(&[("skills", "ponytail"), ("agents", "committer")]);
        let got = scan(d.path(), &Vendor::default()).unwrap();
        assert!(matches!(got["ponytail"], Item::Linked { kind: Kind::Skill, .. }));
        assert!(matches!(got["committer"], Item::Linked { kind: Kind::Agent, .. }));
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn missing_kind_directory_is_not_an_error() {
        let d = repo(&[("skills", "ponytail")]);
        let got = scan(d.path(), &Vendor::default()).unwrap();
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn includes_installers_from_vendor() {
        let d = repo(&[("skills", "ponytail")]);
        let got = scan(d.path(), &vendor_with_installer("bmad", Scope::Project)).unwrap();
        match &got["bmad"] {
            Item::Installer { command, scope, .. } => {
                assert_eq!(command, "echo hi");
                assert_eq!(*scope, Scope::Project);
            }
            other => panic!("expected Installer, got {other:?}"),
        }
    }

    #[test]
    fn rejects_a_name_claimed_by_two_kinds() {
        let d = repo(&[("skills", "clash"), ("agents", "clash")]);
        let err = scan(d.path(), &Vendor::default()).unwrap_err().to_string();
        assert!(err.contains("clash"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_an_installer_shadowing_a_skill() {
        let d = repo(&[("skills", "bmad")]);
        let err = scan(d.path(), &vendor_with_installer("bmad", Scope::Project))
            .unwrap_err()
            .to_string();
        assert!(err.contains("bmad"), "unexpected error: {err}");
    }

    #[test]
    fn ignores_files_and_dotdirs_at_the_kind_level() {
        let d = repo(&[("skills", "ponytail")]);
        fs::write(d.path().join("skills/README.md"), "x").unwrap();
        fs::create_dir(d.path().join("skills/.cache")).unwrap();
        let got = scan(d.path(), &Vendor::default()).unwrap();
        assert_eq!(got.len(), 1);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p wska item::`
Expected: FAIL — `cannot find function scan`.

- [ ] **Step 3: Write the minimal implementation**

Prepend to `crates/wska/src/item.rs`:

```rust
use crate::cli::Scope;
use crate::config::Vendor;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Skill,
    Agent,
    Mcp,
}

impl Kind {
    pub fn subdir(self) -> &'static str {
        match self {
            Kind::Skill => "skills",
            Kind::Agent => "agents",
            Kind::Mcp => "mcp",
        }
    }

    pub fn all() -> [Kind; 3] {
        [Kind::Skill, Kind::Agent, Kind::Mcp]
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Linked {
        name: String,
        kind: Kind,
        path: PathBuf,
    },
    Installer {
        name: String,
        command: String,
        scope: Scope,
    },
}

impl Item {
    pub fn name(&self) -> &str {
        match self {
            Item::Linked { name, .. } | Item::Installer { name, .. } => name,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("reading {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("item name `{name}` is claimed twice: {first} and {second}")]
    DuplicateName {
        name: String,
        first: String,
        second: String,
    },
}

pub fn scan(repo_root: &Path, vendor: &Vendor) -> Result<BTreeMap<String, Item>, ScanError> {
    let mut out: BTreeMap<String, Item> = BTreeMap::new();

    for kind in Kind::all() {
        let dir = repo_root.join(kind.subdir());
        let read = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(source) => return Err(ScanError::Io { path: dir, source }),
        };
        for entry in read {
            let entry = entry.map_err(|source| ScanError::Io {
                path: dir.clone(),
                source,
            })?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let md = std::fs::symlink_metadata(entry.path()).map_err(|source| ScanError::Io {
                path: entry.path(),
                source,
            })?;
            if !md.is_dir() {
                continue;
            }
            insert(&mut out, Item::Linked {
                name: name.clone(),
                kind,
                path: entry.path(),
            }, kind.subdir())?;
        }
    }

    for (name, installer) in &vendor.installers {
        insert(&mut out, Item::Installer {
            name: name.clone(),
            command: installer.command.clone(),
            scope: installer.scope,
        }, "installers")?;
    }

    Ok(out)
}

fn insert(
    out: &mut BTreeMap<String, Item>,
    item: Item,
    origin: &str,
) -> Result<(), ScanError> {
    let name = item.name().to_string();
    if let Some(existing) = out.get(&name) {
        let first = match existing {
            Item::Linked { kind, .. } => kind.subdir().to_string(),
            Item::Installer { .. } => "installers".to_string(),
        };
        return Err(ScanError::DuplicateName {
            name,
            first,
            second: origin.to_string(),
        });
    }
    out.insert(name, item);
    Ok(())
}
```

Add `mod item;` to `crates/wska/src/main.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p wska item::`
Expected: PASS, 6 tests.

- [ ] **Step 5: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat(wska): scan the repo for skills, agents, mcp and installers

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add crates/wska/src/item.rs crates/wska/src/main.rs && rtk git commit -s -F /tmp/wska-commit.txt'
```

---

### Task 5: Profile resolution

**Files:**
- Create: `crates/wska/src/profile.rs`
- Modify: `crates/wska/src/main.rs` (add `mod profile;`)

**Interfaces:**
- Consumes: `config::Profiles`, `item::Item`.
- Produces: `profile::resolve(profiles: &Profiles, wanted: &[String], items: &BTreeMap<String, Item>) -> Result<BTreeSet<String>, ProfileError>` and `profile::ProfileError`.

**Behaviour:** `all` is reserved and expands to every key of `items`. `include` composes. Cycles, unknown profile names and unknown item names are errors. A user-defined profile named `all` is an error naming the collision — Review Focus 4.

- [ ] **Step 1: Write the failing tests**

`crates/wska/src/profile.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Profile, Profiles};
    use crate::item::{Item, Kind};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn items(names: &[&str]) -> BTreeMap<String, Item> {
        names
            .iter()
            .map(|n| {
                (
                    n.to_string(),
                    Item::Linked {
                        name: n.to_string(),
                        kind: Kind::Skill,
                        path: PathBuf::from("skills").join(n),
                    },
                )
            })
            .collect()
    }

    fn profiles(entries: &[(&str, &[&str], &[&str])]) -> Profiles {
        let mut p = Profiles::default();
        for (name, include, items) in entries {
            p.profiles.insert(
                name.to_string(),
                Profile {
                    include: include.iter().map(|s| s.to_string()).collect(),
                    items: items.iter().map(|s| s.to_string()).collect(),
                },
            );
        }
        p
    }

    fn set(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn resolves_a_single_profile() {
        let p = profiles(&[("base", &[], &["a"])]);
        let got = resolve(&p, &["base".into()], &items(&["a", "b"])).unwrap();
        assert_eq!(got, set(&["a"]));
    }

    #[test]
    fn include_is_transitive_and_deduplicated() {
        let p = profiles(&[
            ("base", &[], &["a"]),
            ("mid", &["base"], &["b"]),
            ("top", &["mid", "base"], &["c"]),
        ]);
        let got = resolve(&p, &["top".into()], &items(&["a", "b", "c"])).unwrap();
        assert_eq!(got, set(&["a", "b", "c"]));
    }

    #[test]
    fn multiple_profiles_union() {
        let p = profiles(&[("x", &[], &["a"]), ("y", &[], &["b"])]);
        let got = resolve(&p, &["x".into(), "y".into()], &items(&["a", "b"])).unwrap();
        assert_eq!(got, set(&["a", "b"]));
    }

    #[test]
    fn all_expands_to_every_item() {
        let p = profiles(&[("base", &[], &["a"])]);
        let got = resolve(&p, &["all".into()], &items(&["a", "b", "c"])).unwrap();
        assert_eq!(got, set(&["a", "b", "c"]));
    }

    #[test]
    fn rejects_include_cycles() {
        let p = profiles(&[("x", &["y"], &[]), ("y", &["x"], &[])]);
        let err = resolve(&p, &["x".into()], &items(&["a"])).unwrap_err().to_string();
        assert!(err.contains("cycle"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_unknown_profile() {
        let p = profiles(&[("x", &[], &["a"])]);
        let err = resolve(&p, &["nope".into()], &items(&["a"])).unwrap_err().to_string();
        assert!(err.contains("nope"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_unknown_item() {
        let p = profiles(&[("x", &[], &["ghost"])]);
        let err = resolve(&p, &["x".into()], &items(&["a"])).unwrap_err().to_string();
        assert!(err.contains("ghost"), "unexpected error: {err}");
    }

    // Review Focus 4
    #[test]
    fn rejects_a_user_defined_profile_named_all() {
        let p = profiles(&[("all", &[], &["a"])]);
        let err = resolve(&p, &["all".into()], &items(&["a", "b"])).unwrap_err().to_string();
        assert!(err.contains("reserved"), "unexpected error: {err}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p wska profile::`
Expected: FAIL — `cannot find function resolve`.

- [ ] **Step 3: Write the minimal implementation**

Prepend to `crates/wska/src/profile.rs`:

```rust
use crate::config::Profiles;
use crate::item::Item;
use std::collections::{BTreeMap, BTreeSet};

pub const RESERVED_ALL: &str = "all";

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("`{RESERVED_ALL}` is a reserved profile name; rename the profile in profiles.toml")]
    ReservedName,
    #[error("unknown profile `{0}`")]
    UnknownProfile(String),
    #[error("profile `{profile}` names unknown item `{item}`")]
    UnknownItem { profile: String, item: String },
    #[error("include cycle: {0}")]
    Cycle(String),
}

pub fn resolve(
    profiles: &Profiles,
    wanted: &[String],
    items: &BTreeMap<String, Item>,
) -> Result<BTreeSet<String>, ProfileError> {
    if profiles.profiles.contains_key(RESERVED_ALL) {
        return Err(ProfileError::ReservedName);
    }

    let mut out = BTreeSet::new();
    for name in wanted {
        if name == RESERVED_ALL {
            out.extend(items.keys().cloned());
            continue;
        }
        let mut stack = Vec::new();
        visit(profiles, name, items, &mut stack, &mut out)?;
    }
    Ok(out)
}

fn visit(
    profiles: &Profiles,
    name: &str,
    items: &BTreeMap<String, Item>,
    stack: &mut Vec<String>,
    out: &mut BTreeSet<String>,
) -> Result<(), ProfileError> {
    if stack.iter().any(|s| s == name) {
        stack.push(name.to_string());
        return Err(ProfileError::Cycle(stack.join(" -> ")));
    }
    let profile = profiles
        .profiles
        .get(name)
        .ok_or_else(|| ProfileError::UnknownProfile(name.to_string()))?;

    stack.push(name.to_string());
    for included in &profile.include {
        visit(profiles, included, items, stack, out)?;
    }
    stack.pop();

    for item in &profile.items {
        if !items.contains_key(item) {
            return Err(ProfileError::UnknownItem {
                profile: name.to_string(),
                item: item.clone(),
            });
        }
        out.insert(item.clone());
    }
    Ok(())
}
```

Add `mod profile;` to `crates/wska/src/main.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p wska profile::`
Expected: PASS, 8 tests.

- [ ] **Step 5: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat(wska): resolve profiles with include composition

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add crates/wska/src/profile.rs crates/wska/src/main.rs && rtk git commit -s -F /tmp/wska-commit.txt'
```

---

### Task 6: Target resolution

**Files:**
- Create: `crates/wska/src/target.rs`
- Modify: `crates/wska/src/main.rs` (add `mod target;`)

**Interfaces:**
- Consumes: `cli::Scope`.
- Produces:
  - `target::resolve(scope: Scope, cwd: &Path, env: &dyn Fn(&str) -> Option<String>) -> Result<PathBuf, TargetError>` returning the canonicalized `.claude` directory.
  - `target::repo_root(start: &Path) -> Result<PathBuf, TargetError>` returning the canonicalized git root.
  - `target::TargetError`

**Behaviour:** `User` → `$CLAUDE_CONFIG_DIR` if set, else `$HOME/.claude`. `Project` → git root's `.claude`, falling back to `cwd/.claude` when there is no git root. The returned path is canonicalized when it exists, so a `~/.claude` that is itself a symlink compares correctly — Review Focus 2. The `env` closure is injected so tests do not mutate process environment.

- [ ] **Step 1: Write the failing tests**

`crates/wska/src/target.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| map.get(k).cloned()
    }

    #[test]
    fn user_scope_prefers_claude_config_dir() {
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("cfg");
        fs::create_dir(&cfg).unwrap();
        let env = env_from(&[("CLAUDE_CONFIG_DIR", cfg.to_str().unwrap())]);
        let got = resolve(Scope::User, d.path(), &env).unwrap();
        assert_eq!(got, cfg.canonicalize().unwrap());
    }

    #[test]
    fn user_scope_falls_back_to_home_dot_claude() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join(".claude")).unwrap();
        let env = env_from(&[("HOME", d.path().to_str().unwrap())]);
        let got = resolve(Scope::User, d.path(), &env).unwrap();
        assert_eq!(got, d.path().join(".claude").canonicalize().unwrap());
    }

    #[test]
    fn user_scope_errors_without_home_or_config_dir() {
        let d = tempfile::tempdir().unwrap();
        let env = env_from(&[]);
        assert!(resolve(Scope::User, d.path(), &env).is_err());
    }

    #[test]
    fn project_scope_uses_the_git_root() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join(".git")).unwrap();
        let deep = d.path().join("a/b");
        fs::create_dir_all(&deep).unwrap();
        let env = env_from(&[]);
        let got = resolve(Scope::Project, &deep, &env).unwrap();
        assert_eq!(got, d.path().canonicalize().unwrap().join(".claude"));
    }

    #[test]
    fn project_scope_falls_back_to_cwd_without_a_git_root() {
        let d = tempfile::tempdir().unwrap();
        let env = env_from(&[]);
        let got = resolve(Scope::Project, d.path(), &env).unwrap();
        assert_eq!(got, d.path().canonicalize().unwrap().join(".claude"));
    }

    // Review Focus 2
    #[test]
    fn a_symlinked_target_directory_is_canonicalized() {
        let d = tempfile::tempdir().unwrap();
        let real = d.path().join("dotfiles/claude");
        fs::create_dir_all(&real).unwrap();
        let link = d.path().join("home/.claude");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(&real, &link).unwrap();
        let env = env_from(&[("HOME", d.path().join("home").to_str().unwrap())]);
        let got = resolve(Scope::User, d.path(), &env).unwrap();
        assert_eq!(got, real.canonicalize().unwrap());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p wska target::`
Expected: FAIL — `cannot find function resolve`.

- [ ] **Step 3: Write the minimal implementation**

Prepend to `crates/wska/src/target.rs`:

```rust
use crate::cli::Scope;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("cannot determine the user config directory: set CLAUDE_CONFIG_DIR or HOME")]
    NoUserDir,
    #[error("canonicalizing {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
}

/// The `.claude` directory for a scope, canonicalized when it already exists
/// so that a symlinked `~/.claude` compares correctly against link targets.
pub fn resolve(
    scope: Scope,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<PathBuf, TargetError> {
    let raw = match scope {
        Scope::User => match env("CLAUDE_CONFIG_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(env("HOME").ok_or(TargetError::NoUserDir)?).join(".claude"),
        },
        Scope::Project => git_root(cwd)?.join(".claude"),
    };
    Ok(canonical_or_as_is(&raw)?)
}

/// The repository root containing `start`, canonicalized. Falls back to
/// `start` itself when no `.git` is found, which keeps the CLI usable in a
/// plain directory.
pub fn repo_root(start: &Path) -> Result<PathBuf, TargetError> {
    git_root(start)
}

fn git_root(start: &Path) -> Result<PathBuf, TargetError> {
    let start = canonical_or_as_is(start)?;
    let mut cur = start.as_path();
    loop {
        if cur.join(".git").exists() {
            return Ok(cur.to_path_buf());
        }
        match cur.parent() {
            Some(parent) => cur = parent,
            None => return Ok(start),
        }
    }
}

fn canonical_or_as_is(path: &Path) -> Result<PathBuf, TargetError> {
    match path.canonicalize() {
        Ok(p) => Ok(p),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Not yet created: canonicalize the deepest existing ancestor so
            // the comparison base is still symlink-free.
            match path.parent() {
                Some(parent) => {
                    let base = canonical_or_as_is(parent)?;
                    Ok(base.join(path.file_name().unwrap_or_default()))
                }
                None => Ok(path.to_path_buf()),
            }
        }
        Err(source) => Err(TargetError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}
```

Add `mod target;` to `crates/wska/src/main.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p wska target::`
Expected: PASS, 6 tests.

- [ ] **Step 5: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat(wska): resolve install targets for project and user scope

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add crates/wska/src/target.rs crates/wska/src/main.rs && rtk git commit -s -F /tmp/wska-commit.txt'
```

---

### Task 7: Ownership classification

**Files:**
- Create: `crates/wska/src/ownership.rs`
- Modify: `crates/wska/src/main.rs` (add `mod ownership;`)

**Interfaces:**
- Consumes: nothing beyond std.
- Produces:
  ```rust
  pub enum Entry {
      Absent,
      OwnedCorrect,
      OwnedWrongTarget { points_to: PathBuf },
      OwnedDangling { points_to: PathBuf },
      Foreign { points_to: PathBuf },
      RealPath,
  }
  pub fn classify(entry: &Path, repo_root: &Path, expected: Option<&Path>) -> std::io::Result<Entry>
  ```

**Behaviour:** A relative link target resolves against the link's **parent directory**, never the process working directory — Review Focus 1. Owned means the resolved target is inside `repo_root`. `expected` is `Some(path)` when the plan wants this entry linked to that item, `None` when the plan does not want it at all.

- [ ] **Step 1: Write the failing tests**

`crates/wska/src/ownership.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    struct Fixture {
        _dir: tempfile::TempDir,
        repo: PathBuf,
        target: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let target = dir.path().join("proj/.claude/skills");
        fs::create_dir_all(repo.join("skills/ponytail")).unwrap();
        fs::create_dir_all(repo.join("skills/other")).unwrap();
        fs::create_dir_all(&target).unwrap();
        Fixture {
            _dir: dir,
            repo: repo.canonicalize().unwrap(),
            target: target.canonicalize().unwrap(),
        }
    }

    #[test]
    fn absent_when_nothing_is_there() {
        let f = fixture();
        let got = classify(
            &f.target.join("ponytail"),
            &f.repo,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(matches!(got, Entry::Absent));
    }

    #[test]
    fn owned_correct_for_an_absolute_link_to_the_expected_item() {
        let f = fixture();
        symlink(f.repo.join("skills/ponytail"), f.target.join("ponytail")).unwrap();
        let got = classify(
            &f.target.join("ponytail"),
            &f.repo,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(matches!(got, Entry::OwnedCorrect));
    }

    // Review Focus 1
    #[test]
    fn owned_correct_for_a_relative_link_resolved_against_the_link_parent() {
        let f = fixture();
        let rel = pathdiff_relative(&f.target, &f.repo.join("skills/ponytail"));
        symlink(&rel, f.target.join("ponytail")).unwrap();
        let got = classify(
            &f.target.join("ponytail"),
            &f.repo,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(
            matches!(got, Entry::OwnedCorrect),
            "relative link misclassified as {got:?}"
        );
    }

    #[test]
    fn owned_wrong_target_when_it_points_at_another_item_here() {
        let f = fixture();
        symlink(f.repo.join("skills/other"), f.target.join("ponytail")).unwrap();
        let got = classify(
            &f.target.join("ponytail"),
            &f.repo,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(matches!(got, Entry::OwnedWrongTarget { .. }));
    }

    #[test]
    fn owned_dangling_when_the_target_under_the_repo_is_gone() {
        let f = fixture();
        symlink(f.repo.join("skills/gone"), f.target.join("gone")).unwrap();
        let got = classify(&f.target.join("gone"), &f.repo, None).unwrap();
        assert!(matches!(got, Entry::OwnedDangling { .. }));
    }

    #[test]
    fn foreign_for_a_link_outside_the_repo() {
        let f = fixture();
        let outside = f.repo.parent().unwrap().join("elsewhere");
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, f.target.join("ponytail")).unwrap();
        let got = classify(
            &f.target.join("ponytail"),
            &f.repo,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(matches!(got, Entry::Foreign { .. }));
    }

    #[test]
    fn real_path_for_a_hand_written_directory() {
        let f = fixture();
        fs::create_dir(f.target.join("handmade")).unwrap();
        let got = classify(&f.target.join("handmade"), &f.repo, None).unwrap();
        assert!(matches!(got, Entry::RealPath));
    }

    /// Minimal relative-path helper for the test above; the implementation
    /// does not need one because it only ever writes absolute links.
    fn pathdiff_relative(from: &Path, to: &Path) -> PathBuf {
        let mut up = PathBuf::new();
        let mut base = from.to_path_buf();
        loop {
            if let Ok(rest) = to.strip_prefix(&base) {
                return up.join(rest);
            }
            if !base.pop() {
                return to.to_path_buf();
            }
            up.push("..");
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p wska ownership::`
Expected: FAIL — `cannot find function classify`.

- [ ] **Step 3: Write the minimal implementation**

Prepend to `crates/wska/src/ownership.rs`:

```rust
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub enum Entry {
    /// Nothing exists at this path.
    Absent,
    /// A link we own, already pointing at the expected item.
    OwnedCorrect,
    /// A link we own, pointing at a different item in this repo.
    OwnedWrongTarget { points_to: PathBuf },
    /// A link whose stored target is under this repo but no longer exists.
    OwnedDangling { points_to: PathBuf },
    /// A link pointing outside this repo. Never touched.
    Foreign { points_to: PathBuf },
    /// A real file or directory, not a link. Never touched.
    RealPath,
}

pub fn classify(entry: &Path, repo_root: &Path, expected: Option<&Path>) -> io::Result<Entry> {
    let md = match std::fs::symlink_metadata(entry) {
        Ok(md) => md,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Entry::Absent),
        Err(e) => return Err(e),
    };

    if !md.file_type().is_symlink() {
        return Ok(Entry::RealPath);
    }

    let stored = std::fs::read_link(entry)?;
    // A relative target is relative to the link's own directory, not to the
    // process working directory.
    let resolved = if stored.is_absolute() {
        stored.clone()
    } else {
        normalize(&entry.parent().unwrap_or(Path::new("/")).join(&stored))
    };

    if !resolved.starts_with(repo_root) {
        return Ok(Entry::Foreign { points_to: resolved });
    }
    if !resolved.exists() {
        return Ok(Entry::OwnedDangling { points_to: resolved });
    }
    match expected {
        Some(want) if normalize(want) == resolved => Ok(Entry::OwnedCorrect),
        _ => Ok(Entry::OwnedWrongTarget { points_to: resolved }),
    }
}

/// Lexical normalization: resolves `.` and `..` without touching the
/// filesystem, so a relative link into a repo is comparable to an absolute
/// repo path.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
```

Add `mod ownership;` to `crates/wska/src/main.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p wska ownership::`
Expected: PASS, 7 tests.

- [ ] **Step 5: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat(wska): classify target entries to protect foreign paths

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add crates/wska/src/ownership.rs crates/wska/src/main.rs && rtk git commit -s -F /tmp/wska-commit.txt'
```

---

### Task 8: Plan building and validation

**Files:**
- Create: `crates/wska/src/plan.rs`
- Modify: `crates/wska/src/main.rs` (add `mod plan;`)

**Interfaces:**
- Consumes: `item::{Item, Kind}`, `ownership::{classify, Entry}`, `cli::Scope`.
- Produces:
  ```rust
  pub enum Action {
      Link { from: PathBuf, to: PathBuf },
      Unlink { at: PathBuf },
      RunInstaller { name: String, command: String, cwd: PathBuf },
      SkipInstaller { name: String, reason: String },
  }
  pub struct Refusal { pub path: PathBuf, pub reason: String }
  pub struct Plan { pub actions: Vec<Action>, pub refusals: Vec<Refusal> }
  pub fn build(
      items: &BTreeMap<String, Item>,
      wanted: &BTreeSet<String>,
      target: &Path,
      repo_root: &Path,
      scope: Scope,
  ) -> std::io::Result<Plan>
  impl Plan { pub fn validate(&self) -> Result<(), PlanError> }
  ```

**Behaviour:** declarative — an owned link in a kind subdirectory whose name is not in `wanted` becomes `Unlink`. `Foreign` and `RealPath` entries for wanted items become refusals; the same states for unwanted items are left alone silently, because the CLI has no claim on them. A scope-mismatched installer becomes `SkipInstaller`.

- [ ] **Step 1: Write the failing tests**

`crates/wska/src/plan.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::Kind;
    use std::fs;
    use std::os::unix::fs::symlink;

    struct Fx {
        _dir: tempfile::TempDir,
        repo: PathBuf,
        target: PathBuf,
    }

    fn fx(skills: &[&str]) -> Fx {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        for s in skills {
            fs::create_dir_all(repo.join("skills").join(s)).unwrap();
        }
        let target = dir.path().join("proj/.claude");
        fs::create_dir_all(target.join("skills")).unwrap();
        Fx {
            _dir: dir,
            repo: repo.canonicalize().unwrap(),
            target: target.canonicalize().unwrap(),
        }
    }

    fn linked(repo: &Path, name: &str) -> Item {
        Item::Linked {
            name: name.to_string(),
            kind: Kind::Skill,
            path: repo.join("skills").join(name),
        }
    }

    fn items(repo: &Path, names: &[&str]) -> BTreeMap<String, Item> {
        names
            .iter()
            .map(|n| (n.to_string(), linked(repo, n)))
            .collect()
    }

    fn set(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn links_a_wanted_item_that_is_absent() {
        let f = fx(&["ponytail"]);
        let plan = build(
            &items(&f.repo, &["ponytail"]),
            &set(&["ponytail"]),
            &f.target,
            &f.repo,
            Scope::Project,
        )
        .unwrap();
        assert_eq!(plan.actions.len(), 1);
        assert!(matches!(plan.actions[0], Action::Link { .. }));
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn makes_no_action_for_an_already_correct_link() {
        let f = fx(&["ponytail"]);
        symlink(
            f.repo.join("skills/ponytail"),
            f.target.join("skills/ponytail"),
        )
        .unwrap();
        let plan = build(
            &items(&f.repo, &["ponytail"]),
            &set(&["ponytail"]),
            &f.target,
            &f.repo,
            Scope::Project,
        )
        .unwrap();
        assert!(plan.actions.is_empty(), "{:?}", plan.actions);
    }

    #[test]
    fn unlinks_an_owned_link_that_is_no_longer_wanted() {
        let f = fx(&["ponytail", "other"]);
        symlink(f.repo.join("skills/other"), f.target.join("skills/other")).unwrap();
        let plan = build(
            &items(&f.repo, &["ponytail", "other"]),
            &set(&["ponytail"]),
            &f.target,
            &f.repo,
            Scope::Project,
        )
        .unwrap();
        assert!(plan.actions.iter().any(|a| matches!(a, Action::Unlink { .. })));
    }

    #[test]
    fn refuses_a_real_directory_occupying_a_wanted_path() {
        let f = fx(&["ponytail"]);
        fs::create_dir(f.target.join("skills/ponytail")).unwrap();
        let plan = build(
            &items(&f.repo, &["ponytail"]),
            &set(&["ponytail"]),
            &f.target,
            &f.repo,
            Scope::Project,
        )
        .unwrap();
        assert_eq!(plan.refusals.len(), 1);
        assert!(plan.validate().is_err());
    }

    #[test]
    fn refuses_a_foreign_link_occupying_a_wanted_path() {
        let f = fx(&["ponytail"]);
        let outside = f.repo.parent().unwrap().join("elsewhere");
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, f.target.join("skills/ponytail")).unwrap();
        let plan = build(
            &items(&f.repo, &["ponytail"]),
            &set(&["ponytail"]),
            &f.target,
            &f.repo,
            Scope::Project,
        )
        .unwrap();
        assert_eq!(plan.refusals.len(), 1);
        assert!(plan.validate().is_err());
    }

    #[test]
    fn leaves_an_unwanted_foreign_entry_alone_without_refusing() {
        let f = fx(&["ponytail"]);
        fs::create_dir(f.target.join("skills/handmade")).unwrap();
        let plan = build(
            &items(&f.repo, &["ponytail"]),
            &set(&["ponytail"]),
            &f.target,
            &f.repo,
            Scope::Project,
        )
        .unwrap();
        assert!(plan.refusals.is_empty(), "{:?}", plan.refusals);
        assert!(!plan
            .actions
            .iter()
            .any(|a| matches!(a, Action::Unlink { at } if at.ends_with("handmade"))));
    }

    #[test]
    fn runs_a_project_installer_at_project_scope() {
        let f = fx(&[]);
        let mut map = BTreeMap::new();
        map.insert(
            "bmad".to_string(),
            Item::Installer {
                name: "bmad".into(),
                command: "echo hi".into(),
                scope: Scope::Project,
            },
        );
        let plan = build(&map, &set(&["bmad"]), &f.target, &f.repo, Scope::Project).unwrap();
        assert!(matches!(plan.actions[0], Action::RunInstaller { .. }));
    }

    #[test]
    fn skips_a_project_installer_at_user_scope_without_erroring() {
        let f = fx(&[]);
        let mut map = BTreeMap::new();
        map.insert(
            "bmad".to_string(),
            Item::Installer {
                name: "bmad".into(),
                command: "echo hi".into(),
                scope: Scope::Project,
            },
        );
        let plan = build(&map, &set(&["bmad"]), &f.target, &f.repo, Scope::User).unwrap();
        assert!(matches!(plan.actions[0], Action::SkipInstaller { .. }));
        assert!(plan.validate().is_ok());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p wska plan::`
Expected: FAIL — `cannot find function build`.

- [ ] **Step 3: Write the minimal implementation**

Prepend to `crates/wska/src/plan.rs`:

```rust
use crate::cli::Scope;
use crate::item::{Item, Kind};
use crate::ownership::{classify, Entry};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum Action {
    Link { from: PathBuf, to: PathBuf },
    Unlink { at: PathBuf },
    RunInstaller { name: String, command: String, cwd: PathBuf },
    SkipInstaller { name: String, reason: String },
}

#[derive(Debug)]
pub struct Refusal {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub actions: Vec<Action>,
    pub refusals: Vec<Refusal>,
}

#[derive(Debug, thiserror::Error)]
#[error("refusing to change {count} path(s) this tool does not own")]
pub struct PlanError {
    pub count: usize,
}

impl Plan {
    pub fn validate(&self) -> Result<(), PlanError> {
        if self.refusals.is_empty() {
            Ok(())
        } else {
            Err(PlanError {
                count: self.refusals.len(),
            })
        }
    }
}

pub fn build(
    items: &BTreeMap<String, Item>,
    wanted: &BTreeSet<String>,
    target: &Path,
    repo_root: &Path,
    scope: Scope,
) -> io::Result<Plan> {
    let mut plan = Plan::default();

    // Wanted items: link what is missing, refuse what we do not own.
    for name in wanted {
        let Some(item) = items.get(name) else { continue };
        match item {
            Item::Linked { kind, path, .. } => {
                let at = target.join(kind.subdir()).join(name);
                match classify(&at, repo_root, Some(path))? {
                    Entry::OwnedCorrect => {}
                    Entry::Absent | Entry::OwnedWrongTarget { .. } | Entry::OwnedDangling { .. } => {
                        plan.actions.push(Action::Link {
                            from: path.clone(),
                            to: at,
                        });
                    }
                    Entry::Foreign { points_to } => plan.refusals.push(Refusal {
                        path: at,
                        reason: format!("symlink points outside this repo, at {}", points_to.display()),
                    }),
                    Entry::RealPath => plan.refusals.push(Refusal {
                        path: at,
                        reason: "a real file or directory is already there".to_string(),
                    }),
                }
            }
            Item::Installer { command, scope: want, .. } => {
                if *want == scope {
                    plan.actions.push(Action::RunInstaller {
                        name: name.clone(),
                        command: command.clone(),
                        cwd: target
                            .parent()
                            .unwrap_or(target)
                            .to_path_buf(),
                    });
                } else {
                    plan.actions.push(Action::SkipInstaller {
                        name: name.clone(),
                        reason: format!("declared for {want:?} scope, invoked at {scope:?}"),
                    });
                }
            }
        }
    }

    // Owned links we no longer want: remove them. Anything we do not own is
    // left alone and not reported, since the plan makes no claim on it.
    for kind in Kind::all() {
        let dir = target.join(kind.subdir());
        let read = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        for entry in read {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if wanted.contains(&name) {
                continue;
            }
            match classify(&entry.path(), repo_root, None)? {
                Entry::OwnedCorrect | Entry::OwnedWrongTarget { .. } | Entry::OwnedDangling { .. } => {
                    plan.actions.push(Action::Unlink { at: entry.path() });
                }
                _ => {}
            }
        }
    }

    Ok(plan)
}
```

Add `mod plan;` to `crates/wska/src/main.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p wska plan::`
Expected: PASS, 8 tests.

- [ ] **Step 5: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat(wska): build and validate a declarative install plan

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add crates/wska/src/plan.rs crates/wska/src/main.rs && rtk git commit -s -F /tmp/wska-commit.txt'
```

---

### Task 9: Apply, and the `install` command end to end

**Files:**
- Create: `crates/wska/src/apply.rs`
- Create: `crates/wska/src/app.rs`
- Modify: `crates/wska/src/main.rs`
- Create: `crates/wska/tests/install.rs`

**Interfaces:**
- Consumes: `plan::{Plan, Action}`, `config`, `item`, `profile`, `target`.
- Produces:
  - `apply::apply(plan: &Plan, dry_run: bool) -> Result<Applied, ApplyError>`
  - `apply::Applied { pub linked: usize, pub unlinked: usize, pub installers_run: Vec<String>, pub skipped: Vec<String> }`
  - `app::install(repo_root: &Path, profiles: &[String], scope: Scope, dry_run: bool, cwd: &Path, env: &dyn Fn(&str) -> Option<String>) -> Result<Applied, AppError>`

**Behaviour:** `apply` creates parent directories, writes absolute symlinks, and replaces an existing owned link by removing it first — which is what makes a re-run after an interrupted apply converge rather than fail (Review Focus 3). `dry_run` prints and writes nothing.

- [ ] **Step 1: Write the failing unit tests for apply**

`crates/wska/src/apply.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{Action, Plan};
    use std::fs;
    use std::os::unix::fs::symlink;

    fn plan_linking(from: &Path, to: &Path) -> Plan {
        Plan {
            actions: vec![Action::Link {
                from: from.to_path_buf(),
                to: to.to_path_buf(),
            }],
            refusals: vec![],
        }
    }

    #[test]
    fn creates_the_link_and_its_parent_directory() {
        let d = tempfile::tempdir().unwrap();
        let from = d.path().join("repo/skills/ponytail");
        fs::create_dir_all(&from).unwrap();
        let to = d.path().join("proj/.claude/skills/ponytail");
        let got = apply(&plan_linking(&from, &to), false).unwrap();
        assert_eq!(got.linked, 1);
        assert_eq!(fs::read_link(&to).unwrap(), from);
    }

    #[test]
    fn dry_run_writes_nothing() {
        let d = tempfile::tempdir().unwrap();
        let from = d.path().join("repo/skills/ponytail");
        fs::create_dir_all(&from).unwrap();
        let to = d.path().join("proj/.claude/skills/ponytail");
        let got = apply(&plan_linking(&from, &to), true).unwrap();
        assert_eq!(got.linked, 1);
        assert!(!to.exists());
        assert!(!to.parent().unwrap().exists());
    }

    // Review Focus 3
    #[test]
    fn replaces_an_existing_link_so_a_rerun_converges() {
        let d = tempfile::tempdir().unwrap();
        let from = d.path().join("repo/skills/ponytail");
        let stale = d.path().join("repo/skills/other");
        fs::create_dir_all(&from).unwrap();
        fs::create_dir_all(&stale).unwrap();
        let to = d.path().join("proj/.claude/skills/ponytail");
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        symlink(&stale, &to).unwrap();

        let got = apply(&plan_linking(&from, &to), false).unwrap();
        assert_eq!(got.linked, 1);
        assert_eq!(fs::read_link(&to).unwrap(), from);
    }

    #[test]
    fn unlink_removes_the_link_only() {
        let d = tempfile::tempdir().unwrap();
        let from = d.path().join("repo/skills/ponytail");
        fs::create_dir_all(&from).unwrap();
        let to = d.path().join("proj/.claude/skills/ponytail");
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        symlink(&from, &to).unwrap();

        let plan = Plan {
            actions: vec![Action::Unlink { at: to.clone() }],
            refusals: vec![],
        };
        let got = apply(&plan, false).unwrap();
        assert_eq!(got.unlinked, 1);
        assert!(!to.exists());
        assert!(from.exists(), "the item itself must survive");
    }

    #[test]
    fn runs_an_installer_command() {
        let d = tempfile::tempdir().unwrap();
        let cwd = d.path().join("proj");
        fs::create_dir_all(&cwd).unwrap();
        let plan = Plan {
            actions: vec![Action::RunInstaller {
                name: "touchy".into(),
                command: "touch marker".into(),
                cwd: cwd.clone(),
            }],
            refusals: vec![],
        };
        let got = apply(&plan, false).unwrap();
        assert_eq!(got.installers_run, vec!["touchy".to_string()]);
        assert!(cwd.join("marker").exists());
    }

    #[test]
    fn reports_a_failing_installer_as_an_error() {
        let d = tempfile::tempdir().unwrap();
        let plan = Plan {
            actions: vec![Action::RunInstaller {
                name: "nope".into(),
                command: "exit 3".into(),
                cwd: d.path().to_path_buf(),
            }],
            refusals: vec![],
        };
        assert!(apply(&plan, false).is_err());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p wska apply::`
Expected: FAIL — `cannot find function apply`.

- [ ] **Step 3: Write the minimal apply implementation**

Prepend to `crates/wska/src/apply.rs`:

```rust
use crate::plan::{Action, Plan};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    pub linked: usize,
    pub unlinked: usize,
    pub installers_run: Vec<String>,
    pub skipped: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("{op} {path}: {source}")]
    Io {
        op: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("installer `{name}` failed with status {status}")]
    Installer { name: String, status: String },
}

pub fn apply(plan: &Plan, dry_run: bool) -> Result<Applied, ApplyError> {
    let mut done = Applied::default();

    for action in &plan.actions {
        match action {
            Action::Link { from, to } => {
                println!("link   {} -> {}", to.display(), from.display());
                if !dry_run {
                    if let Some(parent) = to.parent() {
                        std::fs::create_dir_all(parent).map_err(|source| ApplyError::Io {
                            op: "creating",
                            path: parent.to_path_buf(),
                            source,
                        })?;
                    }
                    // Remove first so that replacing an existing link is
                    // idempotent; symlink() refuses an occupied path.
                    remove_if_link(to)?;
                    std::os::unix::fs::symlink(from, to).map_err(|source| ApplyError::Io {
                        op: "linking",
                        path: to.clone(),
                        source,
                    })?;
                }
                done.linked += 1;
            }
            Action::Unlink { at } => {
                println!("unlink {}", at.display());
                if !dry_run {
                    remove_if_link(at)?;
                }
                done.unlinked += 1;
            }
            Action::RunInstaller { name, command, cwd } => {
                println!("run    {name}: {command}");
                if !dry_run {
                    let status = Command::new("sh")
                        .arg("-c")
                        .arg(command)
                        .current_dir(cwd)
                        .status()
                        .map_err(|source| ApplyError::Io {
                            op: "spawning installer in",
                            path: cwd.clone(),
                            source,
                        })?;
                    if !status.success() {
                        return Err(ApplyError::Installer {
                            name: name.clone(),
                            status: status.to_string(),
                        });
                    }
                }
                done.installers_run.push(name.clone());
            }
            Action::SkipInstaller { name, reason } => {
                println!("skip   {name}: {reason}");
                done.skipped.push(name.clone());
            }
        }
    }

    Ok(done)
}

fn remove_if_link(path: &Path) -> Result<(), ApplyError> {
    match std::fs::symlink_metadata(path) {
        Ok(md) if md.file_type().is_symlink() => {
            std::fs::remove_file(path).map_err(|source| ApplyError::Io {
                op: "removing",
                path: path.to_path_buf(),
                source,
            })
        }
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ApplyError::Io {
            op: "inspecting",
            path: path.to_path_buf(),
            source,
        }),
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p wska apply::`
Expected: PASS, 6 tests.

- [ ] **Step 5: Write the failing integration test for the whole command**

`crates/wska/tests/install.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

fn repo(root: &Path, skills: &[&str], profiles_toml: &str) {
    for s in skills {
        let d = root.join("skills").join(s);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("SKILL.md"), "x").unwrap();
    }
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join("profiles.toml"), profiles_toml).unwrap();
    fs::write(root.join("vendor.toml"), "").unwrap();
}

fn env_none(_: &str) -> Option<String> {
    None
}

#[test]
fn installs_a_profile_then_converges_on_a_second_run() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["ponytail", "graphify", "impeccable"],
        r#"
[profiles.base]
items = ["ponytail", "graphify"]

[profiles.frontend]
include = ["base"]
items = ["impeccable"]
"#,
    );

    let first = wska::app::install(
        &root,
        &["frontend".to_string()],
        wska::cli::Scope::Project,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(first.linked, 3);

    let links: PathBuf = root.join(".claude/skills");
    for s in ["ponytail", "graphify", "impeccable"] {
        assert!(fs::symlink_metadata(links.join(s)).unwrap().file_type().is_symlink());
    }

    // Idempotent
    let again = wska::app::install(
        &root,
        &["frontend".to_string()],
        wska::cli::Scope::Project,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(again.linked, 0);
    assert_eq!(again.unlinked, 0);

    // Declarative: narrowing the profile set removes what is no longer wanted
    let narrowed = wska::app::install(
        &root,
        &["base".to_string()],
        wska::cli::Scope::Project,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(narrowed.unlinked, 1);
    assert!(!links.join("impeccable").exists());
    assert!(links.join("ponytail").exists());
}

#[test]
fn refuses_to_replace_a_hand_written_directory_and_changes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["ponytail", "graphify"],
        "[profiles.base]\nitems = [\"ponytail\", \"graphify\"]\n",
    );
    let handmade = root.join(".claude/skills/ponytail");
    fs::create_dir_all(&handmade).unwrap();
    fs::write(handmade.join("SKILL.md"), "mine").unwrap();

    let err = wska::app::install(
        &root,
        &["base".to_string()],
        wska::cli::Scope::Project,
        false,
        &root,
        &env_none,
    )
    .unwrap_err();
    assert!(err.to_string().contains("own"), "unexpected error: {err}");

    // Nothing was written, including the item that would have succeeded.
    assert_eq!(fs::read_to_string(handmade.join("SKILL.md")).unwrap(), "mine");
    assert!(!root.join(".claude/skills/graphify").exists());
}
```

- [ ] **Step 6: Run the integration test to verify it fails**

Run: `rtk cargo test -p wska --test install`
Expected: FAIL — `unresolved import wska::app` (the crate has no library target yet).

- [ ] **Step 7: Add a library target and the `app` layer**

`crates/wska/Cargo.toml`, add:

```toml
[lib]
name = "wska"
path = "src/lib.rs"

[[bin]]
name = "wska"
path = "src/main.rs"
```

`crates/wska/src/lib.rs`:

```rust
pub mod app;
pub mod apply;
pub mod cli;
pub mod config;
pub mod hash;
pub mod item;
pub mod ownership;
pub mod plan;
pub mod profile;
pub mod target;
```

`crates/wska/src/app.rs`:

```rust
use crate::apply::{apply, Applied, ApplyError};
use crate::cli::Scope;
use crate::{config, item, plan, profile, target};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    Scan(#[from] item::ScanError),
    #[error(transparent)]
    Profile(#[from] profile::ProfileError),
    #[error(transparent)]
    Target(#[from] target::TargetError),
    #[error(transparent)]
    Apply(#[from] ApplyError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Refused(String),
}

pub fn install(
    repo_root: &Path,
    profiles: &[String],
    scope: Scope,
    dry_run: bool,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Applied, AppError> {
    let profiles_cfg = config::load_profiles(&repo_root.join("profiles.toml"))?;
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    let items = item::scan(repo_root, &vendor)?;
    let wanted = profile::resolve(&profiles_cfg, profiles, &items)?;

    let repo_root = target::repo_root(repo_root)?;
    let target_dir = target::resolve(scope, cwd, env)?;

    let built = plan::build(&items, &wanted, &target_dir, &repo_root, scope)?;
    if let Err(e) = built.validate() {
        for refusal in &built.refusals {
            eprintln!("refuse {}: {}", refusal.path.display(), refusal.reason);
        }
        return Err(AppError::Refused(e.to_string()));
    }
    Ok(apply(&built, dry_run)?)
}
```

`crates/wska/src/main.rs` becomes:

```rust
use clap::Parser;
use wska::cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let cwd = std::env::current_dir().expect("current directory");
    let env = |k: &str| std::env::var(k).ok();

    let result = match cli.command {
        Command::Install { profile, scope, dry_run } => {
            let root = wska::target::repo_root(&cwd).expect("repo root");
            wska::app::install(&root, &profile, scope, dry_run, &cwd, &env).map(|a| {
                println!(
                    "linked {} unlinked {} installers {} skipped {}",
                    a.linked,
                    a.unlinked,
                    a.installers_run.len(),
                    a.skipped.len()
                );
            })
        }
        other => {
            eprintln!("not implemented: {other:?}");
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
```

Delete the `mod` declarations from `main.rs`; the binary now uses the library.

- [ ] **Step 8: Run every test**

Run: `rtk cargo test -p wska`
Expected: PASS, all unit tests plus 2 integration tests.

- [ ] **Step 9: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat(wska): apply install plans and wire the install command

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add crates/wska && rtk git commit -s -F /tmp/wska-commit.txt'
```

---

### Task 10: `status` and `list`

**Files:**
- Create: `crates/wska/src/status.rs`
- Modify: `crates/wska/src/lib.rs` (add `pub mod status;`)
- Modify: `crates/wska/src/app.rs` (add `status` and `list`)
- Modify: `crates/wska/src/main.rs` (wire both commands)

**Interfaces:**
- Consumes: `item`, `ownership`, `target`, `config`.
- Produces:
  - `status::Row { pub name: String, pub state: State }`
  - `status::State { Linked, Dangling, Foreign { points_to: PathBuf }, RealPath, InstallerUnmanaged, MissingFromTarget }`
  - `status::collect(items: &BTreeMap<String, Item>, target: &Path, repo_root: &Path) -> std::io::Result<Vec<Row>>`
  - `app::status(...) -> Result<Vec<status::Row>, AppError>`
  - `app::list(repo_root: &Path, profiles: bool) -> Result<Vec<String>, AppError>`

- [ ] **Step 1: Write the failing tests**

`crates/wska/src/status.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Scope;
    use crate::item::{Item, Kind};
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn state_of(rows: &[Row], name: &str) -> String {
        format!("{:?}", rows.iter().find(|r| r.name == name).unwrap().state)
    }

    #[test]
    fn reports_each_state() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        let target = d.path().join("proj/.claude");
        for s in ["ok", "dangly", "alien", "handmade", "absent"] {
            fs::create_dir_all(repo.join("skills").join(s)).unwrap();
        }
        fs::create_dir_all(target.join("skills")).unwrap();
        let repo = repo.canonicalize().unwrap();
        let target = target.canonicalize().unwrap();

        symlink(repo.join("skills/ok"), target.join("skills/ok")).unwrap();
        symlink(repo.join("skills/vanished"), target.join("skills/dangly")).unwrap();
        let outside = d.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, target.join("skills/alien")).unwrap();
        fs::create_dir(target.join("skills/handmade")).unwrap();

        let mut items: BTreeMap<String, Item> = ["ok", "dangly", "alien", "handmade", "absent"]
            .iter()
            .map(|n| {
                (
                    n.to_string(),
                    Item::Linked {
                        name: n.to_string(),
                        kind: Kind::Skill,
                        path: repo.join("skills").join(n),
                    },
                )
            })
            .collect();
        items.insert(
            "bmad".into(),
            Item::Installer {
                name: "bmad".into(),
                command: "true".into(),
                scope: Scope::Project,
            },
        );

        let rows = collect(&items, &target, &repo).unwrap();
        assert_eq!(state_of(&rows, "ok"), "Linked");
        assert!(state_of(&rows, "dangly").starts_with("Dangling"));
        assert!(state_of(&rows, "alien").starts_with("Foreign"));
        assert_eq!(state_of(&rows, "handmade"), "RealPath");
        assert_eq!(state_of(&rows, "absent"), "MissingFromTarget");
        assert_eq!(state_of(&rows, "bmad"), "InstallerUnmanaged");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `rtk cargo test -p wska status::`
Expected: FAIL — `cannot find function collect`.

- [ ] **Step 3: Write the minimal implementation**

Prepend to `crates/wska/src/status.rs`:

```rust
use crate::item::Item;
use crate::ownership::{classify, Entry};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum State {
    Linked,
    Dangling { points_to: PathBuf },
    Foreign { points_to: PathBuf },
    RealPath,
    InstallerUnmanaged,
    MissingFromTarget,
}

#[derive(Debug)]
pub struct Row {
    pub name: String,
    pub state: State,
}

pub fn collect(
    items: &BTreeMap<String, Item>,
    target: &Path,
    repo_root: &Path,
) -> io::Result<Vec<Row>> {
    let mut rows = Vec::new();
    for (name, item) in items {
        let state = match item {
            Item::Installer { .. } => State::InstallerUnmanaged,
            Item::Linked { kind, path, .. } => {
                let at = target.join(kind.subdir()).join(name);
                match classify(&at, repo_root, Some(path))? {
                    Entry::OwnedCorrect => State::Linked,
                    Entry::OwnedWrongTarget { .. } => State::MissingFromTarget,
                    Entry::OwnedDangling { points_to } => State::Dangling { points_to },
                    Entry::Foreign { points_to } => State::Foreign { points_to },
                    Entry::RealPath => State::RealPath,
                    Entry::Absent => State::MissingFromTarget,
                }
            }
        };
        rows.push(Row {
            name: name.clone(),
            state,
        });
    }
    Ok(rows)
}
```

Add to `crates/wska/src/app.rs`:

```rust
pub fn status(
    repo_root: &Path,
    scope: Scope,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Vec<crate::status::Row>, AppError> {
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    let items = item::scan(repo_root, &vendor)?;
    let root = target::repo_root(repo_root)?;
    let target_dir = target::resolve(scope, cwd, env)?;
    Ok(crate::status::collect(&items, &target_dir, &root)?)
}

pub fn list(repo_root: &Path, profiles: bool) -> Result<Vec<String>, AppError> {
    if profiles {
        let cfg = config::load_profiles(&repo_root.join("profiles.toml"))?;
        return Ok(cfg.profiles.keys().cloned().collect());
    }
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    Ok(item::scan(repo_root, &vendor)?.keys().cloned().collect())
}
```

Add `pub mod status;` to `crates/wska/src/lib.rs`, and replace the `other =>` arm in `main.rs` with:

```rust
        Command::Status { scope } => {
            let root = wska::target::repo_root(&cwd).expect("repo root");
            wska::app::status(&root, scope, &cwd, &env).map(|rows| {
                for row in rows {
                    println!("{:<28} {:?}", row.name, row.state);
                }
            })
        }
        Command::List { profiles } => {
            let root = wska::target::repo_root(&cwd).expect("repo root");
            wska::app::list(&root, profiles).map(|names| {
                for name in names {
                    println!("{name}");
                }
            })
        }
        Command::Sync { .. } => {
            eprintln!("not implemented: sync");
            std::process::exit(1);
        }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p wska`
Expected: PASS, all tests.

- [ ] **Step 5: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat(wska): add status and list commands

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add crates/wska && rtk git commit -s -F /tmp/wska-commit.txt'
```

---

### Task 11: Sync

**Files:**
- Create: `crates/wska/src/sync.rs`
- Modify: `crates/wska/src/lib.rs` (add `pub mod sync;`)
- Modify: `crates/wska/src/app.rs` (add `sync`)
- Modify: `crates/wska/src/main.rs` (wire `Command::Sync`)
- Modify: `.gitignore` (add the clone cache)

**Interfaces:**
- Consumes: `config::{Source, Provides, Vendor}`, `hash::hash_dir`.
- Produces:
  - `sync::Outcome { FastForward { skill, from, to }, Patched { skill }, Unchanged { skill }, MissingLicence { source } }`
  - `sync::sync_source(name: &str, src: &Source, repo_root: &Path, cache: &Path, check_only: bool) -> Result<Vec<Outcome>, SyncError>`
  - `sync::write_back(vendor_path: &Path, source: &str, commit: &str, hashes: &BTreeMap<String, String>) -> Result<(), SyncError>` using `toml_edit` so comments survive.

**Behaviour:** clone `--depth 1 --branch <ref>` into `cache/<source>`, read the new commit with `git rev-parse --short HEAD`, then per provided skill compare recorded hash, local hash and upstream content. Local equals recorded → overwrite and record. Local differs → `Patched`, nothing written. `check_only` never writes and reports `MissingLicence` when `licence` is `None`.

- [ ] **Step 1: Write the failing tests**

`crates/wska/src/sync.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Provides, Source};
    use std::collections::BTreeMap;
    use std::fs;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    /// An upstream repository with skills/<name>/SKILL.md on branch main.
    fn upstream(dir: &Path, body: &str) {
        fs::create_dir_all(dir.join("skills/brainstorming")).unwrap();
        fs::write(dir.join("skills/brainstorming/SKILL.md"), body).unwrap();
        fs::write(dir.join("LICENSE"), "MIT").unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-qm", "x"]);
    }

    fn source(upstream_dir: &Path) -> Source {
        Source {
            upstream: format!("file://{}", upstream_dir.display()),
            git_ref: "main".into(),
            subpath: "skills".into(),
            provides: Provides::Star("*".into()),
            licence: Some("MIT".into()),
            commit: None,
            hashes: BTreeMap::new(),
        }
    }

    #[test]
    fn fast_forwards_an_unpatched_skill() {
        let d = tempfile::tempdir().unwrap();
        let up = d.path().join("up");
        upstream(&up, "v1");
        let repo = d.path().join("repo");
        let local = repo.join("skills/brainstorming");
        fs::create_dir_all(&local).unwrap();
        fs::write(local.join("SKILL.md"), "v1").unwrap();

        let mut src = source(&up);
        src.hashes.insert(
            "brainstorming".into(),
            crate::hash::hash_dir(&local).unwrap(),
        );
        // upstream moves on
        fs::write(up.join("skills/brainstorming/SKILL.md"), "v2").unwrap();
        git(&up, &["commit", "-aqm", "v2"]);

        let out = sync_source("sp", &src, &repo, &d.path().join("cache"), false).unwrap();
        assert!(
            out.iter().any(|o| matches!(o, Outcome::FastForward { .. })),
            "{out:?}"
        );
        assert_eq!(fs::read_to_string(local.join("SKILL.md")).unwrap(), "v2");
    }

    #[test]
    fn refuses_to_overwrite_a_patched_skill() {
        let d = tempfile::tempdir().unwrap();
        let up = d.path().join("up");
        upstream(&up, "v1");
        let repo = d.path().join("repo");
        let local = repo.join("skills/brainstorming");
        fs::create_dir_all(&local).unwrap();
        fs::write(local.join("SKILL.md"), "v1").unwrap();

        let mut src = source(&up);
        src.hashes.insert(
            "brainstorming".into(),
            crate::hash::hash_dir(&local).unwrap(),
        );
        // the author patches locally
        fs::write(local.join("SKILL.md"), "v1 + my edit").unwrap();
        // and upstream moves on
        fs::write(up.join("skills/brainstorming/SKILL.md"), "v2").unwrap();
        git(&up, &["commit", "-aqm", "v2"]);

        let out = sync_source("sp", &src, &repo, &d.path().join("cache"), false).unwrap();
        assert!(out.iter().any(|o| matches!(o, Outcome::Patched { .. })), "{out:?}");
        assert_eq!(
            fs::read_to_string(local.join("SKILL.md")).unwrap(),
            "v1 + my edit",
            "a patched skill must not be overwritten"
        );
    }

    #[test]
    fn check_only_reports_a_missing_licence_and_writes_nothing() {
        let d = tempfile::tempdir().unwrap();
        let up = d.path().join("up");
        upstream(&up, "v1");
        let repo = d.path().join("repo");
        let local = repo.join("skills/brainstorming");
        fs::create_dir_all(&local).unwrap();
        fs::write(local.join("SKILL.md"), "v1").unwrap();

        let mut src = source(&up);
        src.licence = None;
        src.hashes.insert(
            "brainstorming".into(),
            crate::hash::hash_dir(&local).unwrap(),
        );
        fs::write(up.join("skills/brainstorming/SKILL.md"), "v2").unwrap();
        git(&up, &["commit", "-aqm", "v2"]);

        let out = sync_source("sp", &src, &repo, &d.path().join("cache"), true).unwrap();
        assert!(out.iter().any(|o| matches!(o, Outcome::MissingLicence { .. })), "{out:?}");
        assert_eq!(fs::read_to_string(local.join("SKILL.md")).unwrap(), "v1");
    }

    #[test]
    fn write_back_preserves_comments() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("vendor.toml");
        fs::write(
            &p,
            "# keep me\n[sources.sp]\nupstream = \"x\"\nref = \"main\"\nsubpath = \"skills\"\nprovides = \"*\"\nlicence = \"MIT\"\n",
        )
        .unwrap();
        let mut hashes = BTreeMap::new();
        hashes.insert("brainstorming".to_string(), "sha256:ab".to_string());
        write_back(&p, "sp", "deadbee", &hashes).unwrap();
        let body = fs::read_to_string(&p).unwrap();
        assert!(body.contains("# keep me"), "{body}");
        assert!(body.contains("deadbee"), "{body}");
        assert!(body.contains("sha256:ab"), "{body}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `rtk cargo test -p wska sync::`
Expected: FAIL — `cannot find function sync_source`.

- [ ] **Step 3: Write the minimal implementation**

Prepend to `crates/wska/src/sync.rs`:

```rust
use crate::config::{Provides, Source};
use crate::hash::hash_dir;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub enum Outcome {
    Unchanged { skill: String },
    FastForward { skill: String, from: String, to: String },
    Patched { skill: String },
    MissingLicence { source: String },
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("git {args} failed: {status}")]
    Git { args: String, status: String },
    #[error("{op} {path}: {source}")]
    Io {
        op: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("vendor.toml has no source named `{0}`")]
    UnknownSource(String),
    #[error("parsing vendor.toml: {0}")]
    Edit(#[from] toml_edit::TomlError),
}

pub fn sync_source(
    name: &str,
    src: &Source,
    repo_root: &Path,
    cache: &Path,
    check_only: bool,
) -> Result<Vec<Outcome>, SyncError> {
    let mut out = Vec::new();
    if src.licence.is_none() {
        out.push(Outcome::MissingLicence {
            source: name.to_string(),
        });
    }

    let clone = cache.join(name);
    if clone.exists() {
        std::fs::remove_dir_all(&clone).map_err(|source| SyncError::Io {
            op: "clearing",
            path: clone.clone(),
            source,
        })?;
    }
    std::fs::create_dir_all(cache).map_err(|source| SyncError::Io {
        op: "creating",
        path: cache.to_path_buf(),
        source,
    })?;
    git(
        cache,
        &[
            "clone",
            "--quiet",
            "--depth",
            "1",
            "--branch",
            &src.git_ref,
            &src.upstream,
            name,
        ],
    )?;
    let new_commit = git_stdout(&clone, &["rev-parse", "--short", "HEAD"])?;

    let upstream_dir = clone.join(&src.subpath);
    let skills = match &src.provides {
        Provides::Named(v) => v.clone(),
        Provides::Star(_) => {
            let mut v = Vec::new();
            for entry in std::fs::read_dir(&upstream_dir).map_err(|source| SyncError::Io {
                op: "reading",
                path: upstream_dir.clone(),
                source,
            })? {
                let entry = entry.map_err(|source| SyncError::Io {
                    op: "reading",
                    path: upstream_dir.clone(),
                    source,
                })?;
                if entry.path().is_dir() {
                    v.push(entry.file_name().to_string_lossy().to_string());
                }
            }
            v.sort();
            v
        }
    };

    let mut fresh: BTreeMap<String, String> = BTreeMap::new();
    for skill in skills {
        let local = repo_root.join("skills").join(&skill);
        let remote = upstream_dir.join(&skill);
        let remote_hash = hash_dir(&remote).map_err(|source| SyncError::Io {
            op: "hashing",
            path: remote.clone(),
            source,
        })?;

        if !local.exists() {
            if !check_only {
                copy_dir(&remote, &local)?;
            }
            fresh.insert(skill.clone(), remote_hash.clone());
            out.push(Outcome::FastForward {
                skill,
                from: "absent".into(),
                to: new_commit.clone(),
            });
            continue;
        }

        let local_hash = hash_dir(&local).map_err(|source| SyncError::Io {
            op: "hashing",
            path: local.clone(),
            source,
        })?;
        let recorded = src.hashes.get(&skill);

        if recorded == Some(&local_hash) {
            if local_hash == remote_hash {
                fresh.insert(skill.clone(), local_hash);
                out.push(Outcome::Unchanged { skill });
            } else {
                if !check_only {
                    std::fs::remove_dir_all(&local).map_err(|source| SyncError::Io {
                        op: "replacing",
                        path: local.clone(),
                        source,
                    })?;
                    copy_dir(&remote, &local)?;
                }
                fresh.insert(skill.clone(), remote_hash);
                out.push(Outcome::FastForward {
                    skill,
                    from: src.commit.clone().unwrap_or_else(|| "unknown".into()),
                    to: new_commit.clone(),
                });
            }
        } else {
            // Patched locally: keep the recorded hash so the next sync still
            // sees the patch, and never overwrite.
            if let Some(r) = recorded {
                fresh.insert(skill.clone(), r.clone());
            }
            out.push(Outcome::Patched { skill });
        }
    }

    if !check_only {
        write_back(&repo_root.join("vendor.toml"), name, &new_commit, &fresh)?;
    }
    Ok(out)
}

/// Update one source's `commit` and `hashes` in place, preserving comments
/// and field order.
pub fn write_back(
    vendor_path: &Path,
    source: &str,
    commit: &str,
    hashes: &BTreeMap<String, String>,
) -> Result<(), SyncError> {
    let body = std::fs::read_to_string(vendor_path).map_err(|e| SyncError::Io {
        op: "reading",
        path: vendor_path.to_path_buf(),
        source: e,
    })?;
    let mut doc: toml_edit::DocumentMut = body.parse()?;
    let table = doc
        .get_mut("sources")
        .and_then(|s| s.get_mut(source))
        .ok_or_else(|| SyncError::UnknownSource(source.to_string()))?;

    table["commit"] = toml_edit::value(commit);
    let mut inline = toml_edit::InlineTable::new();
    for (k, v) in hashes {
        inline.insert(k, v.clone().into());
    }
    table["hashes"] = toml_edit::Item::Value(toml_edit::Value::InlineTable(inline));

    std::fs::write(vendor_path, doc.to_string()).map_err(|e| SyncError::Io {
        op: "writing",
        path: vendor_path.to_path_buf(),
        source: e,
    })
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), SyncError> {
    std::fs::create_dir_all(to).map_err(|source| SyncError::Io {
        op: "creating",
        path: to.to_path_buf(),
        source,
    })?;
    for entry in std::fs::read_dir(from).map_err(|source| SyncError::Io {
        op: "reading",
        path: from.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| SyncError::Io {
            op: "reading",
            path: from.to_path_buf(),
            source,
        })?;
        let dest = to.join(entry.file_name());
        let md = std::fs::symlink_metadata(entry.path()).map_err(|source| SyncError::Io {
            op: "inspecting",
            path: entry.path(),
            source,
        })?;
        if md.file_type().is_symlink() {
            let target = std::fs::read_link(entry.path()).map_err(|source| SyncError::Io {
                op: "reading link",
                path: entry.path(),
                source,
            })?;
            std::os::unix::fs::symlink(target, &dest).map_err(|source| SyncError::Io {
                op: "linking",
                path: dest,
                source,
            })?;
        } else if md.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest).map_err(|source| SyncError::Io {
                op: "copying to",
                path: dest,
                source,
            })?;
        }
    }
    Ok(())
}

fn git(cwd: &Path, args: &[&str]) -> Result<(), SyncError> {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|source| SyncError::Io {
            op: "spawning git in",
            path: cwd.to_path_buf(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(SyncError::Git {
            args: args.join(" "),
            status: status.to_string(),
        })
    }
}

fn git_stdout(cwd: &Path, args: &[&str]) -> Result<String, SyncError> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|source| SyncError::Io {
            op: "spawning git in",
            path: cwd.to_path_buf(),
            source,
        })?;
    if !out.status.success() {
        return Err(SyncError::Git {
            args: args.join(" "),
            status: out.status.to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
```

Add to `crates/wska/src/app.rs`:

```rust
pub fn sync(
    repo_root: &Path,
    sources: &[String],
    check: bool,
) -> Result<Vec<crate::sync::Outcome>, AppError> {
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    let cache = repo_root.join(".task/wska-cache");
    let mut out = Vec::new();
    for (name, source) in &vendor.sources {
        if !sources.is_empty() && !sources.contains(name) {
            continue;
        }
        out.extend(crate::sync::sync_source(name, source, repo_root, &cache, check)?);
    }
    Ok(out)
}
```

Add `#[error(transparent)] Sync(#[from] crate::sync::SyncError),` to `AppError`, `pub mod sync;` to `lib.rs`, and wire `Command::Sync` in `main.rs`:

```rust
        Command::Sync { source, check } => {
            let root = wska::target::repo_root(&cwd).expect("repo root");
            wska::app::sync(&root, &source, check).map(|outcomes| {
                let mut bad = false;
                for o in &outcomes {
                    println!("{o:?}");
                    if matches!(o, wska::sync::Outcome::MissingLicence { .. }) {
                        bad = true;
                    }
                }
                if check && bad {
                    std::process::exit(1);
                }
            })
        }
```

`.gitignore`, add:

```
# wska upstream clone cache
.task/wska-cache/
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `rtk cargo test -p wska sync::`
Expected: PASS, 4 tests.

- [ ] **Step 5: Run the whole suite and the linter**

Run: `rtk cargo test -p wska && rtk cargo clippy -- -D warnings && cargo fmt --check`
Expected: all green.

- [ ] **Step 6: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat(wska): sync vendored sources with patch detection

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add crates/wska .gitignore && rtk git commit -s -F /tmp/wska-commit.txt'
```

---

### Task 12: Task wiring

**Files:**
- Create: `.tasks/skills.yaml`
- Modify: `Taskfile.yaml`
- Modify: `mise.toml`

**Interfaces:**
- Consumes: the `wska` binary.
- Produces: `task skills:install`, `skills:status`, `skills:list`, `skills:sync`, `skills:sync:check`, `skills:test`; `task lint` and `task audit` doing real work; `task init` self-installing.

- [ ] **Step 1: Verify cargo-audit is installable**

Run: `rtk mise use cargo:cargo-audit && rtk cargo audit --version`
Expected: a version string. If this fails, use `cargo:cargo-deny` with `cargo deny check advisories` instead and note the substitution in the task report.

- [ ] **Step 2: Create the task file**

`.tasks/skills.yaml`:

```yaml
version: "3"

vars:
  WSKA: '{{.WSKA | default "cargo run --quiet -p wska --"}}'
  SCOPE: '{{.SCOPE | default "project"}}'

tasks:
  build:
    desc: "Build the wska binary"
    cmds:
      - cargo build --release -p wska

  install:
    desc: "Link the items of one or more profiles into a target (usage: task skills:install PROFILE=base [SCOPE=project|user])"
    preconditions:
      - sh: 'test -n "{{.PROFILE}}"'
        msg: "pass PROFILE=<name>, or PROFILE=all for everything"
    cmds:
      - '{{.WSKA}} install --profile {{.PROFILE}} --scope {{.SCOPE}}'

  status:
    desc: "Show what is linked in a target (usage: task skills:status [SCOPE=project|user])"
    cmds:
      - '{{.WSKA}} status --scope {{.SCOPE}}'

  list:
    desc: "List the items in this repo (usage: task skills:list [PROFILES=1])"
    cmds:
      - '{{.WSKA}} list {{if .PROFILES}}--profiles{{end}}'

  sync:
    desc: "Refresh vendored sources from upstream (usage: task skills:sync [SOURCE=superpowers])"
    cmds:
      - '{{.WSKA}} sync {{.SOURCE}}'

  sync:check:
    desc: "Report vendored drift, patched skills and missing licences without writing"
    cmds:
      - '{{.WSKA}} sync --check'

  test:
    desc: "Run the wska test suite"
    cmds:
      - cargo test -p wska
```

- [ ] **Step 3: Wire the root Taskfile**

In `Taskfile.yaml`, add `skills: ./.tasks/skills.yaml` to `includes`, then replace the three placeholder tasks and extend `init`:

```yaml
tasks:
  init:
    desc: "Init folder"
    cmds:
      - task mise:install
      - task claude:install
      - cog install-hook -a -o
      - task skills:install PROFILE=all SCOPE=project

  recu:
    desc: "Recuring job like regenerating Swagger spec"
    cmds:
      - echo "nothing to regenerate yet"

  lint:
    desc: "Linting your project to apply good practice"
    cmds:
      - cargo fmt --all --check
      - cargo clippy --all-targets -- -D warnings

  audit:
    desc: "Check dependencies and Containerfile for CVE"
    cmds:
      - cargo audit
      - task skills:sync:check
```

`mise.toml`, add under `[tools]`:

```toml
"cargo:cargo-audit" = "latest"
```

- [ ] **Step 4: Verify the wiring**

Run: `rtk task skills:list && rtk task skills:status && rtk task lint`
Expected: `skills:list` prints nothing yet (no items vendored until Task 13), `skills:status` prints nothing, `task lint` passes.

The pre-commit hook runs `task recu` and `task lint`, so `task lint` must pass before the author can commit anything from here on.

- [ ] **Step 5: Prepare the commit**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
build: wire wska into Taskfile and mise

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add .tasks/skills.yaml Taskfile.yaml mise.toml mise.lock && rtk git commit -s -F /tmp/wska-commit.txt'
```

---

### Task 13: Vendoring and migration

This task produces the repo's actual content and ends the plugin era. It has **three separate commits**, because step 7 is the irreversible one.

**Files:**
- Create: `skills/<each vendored skill>/…`
- Create: `profiles.toml`
- Create: `vendor.toml`
- Modify: `.gitignore`
- Delete from version control: `.claude/skills/graphify/**`
- Modify: `.claude/settings.json`

**Interfaces:**
- Consumes: `wska sync`, `wska install`.
- Produces: a populated `skills/`, a working `profiles.toml`, and a `settings.json` with no plugin declarations.

- [ ] **Step 1: Stop tracking this repo's own install directory**

```bash
printf '\n# wska links this repo'"'"'s skills in here\n.claude/skills/\n' >> .gitignore
rtk git rm -r --cached .claude/skills
```

Note: `.claude/CLAUDE.md`, `.claude/settings.json` and `.claude/settings.json.graphify-bak` stay tracked. Verify with `rtk git status --short` that only `.claude/skills/**` was unstaged.

- [ ] **Step 2: Commit the gitignore change on its own**

```bash
cat > /tmp/wska-commit.txt <<'MSG'
chore: stop tracking .claude/skills, wska populates it

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add .gitignore .claude && rtk git commit -s -F /tmp/wska-commit.txt'
```

- [ ] **Step 3: Write `vendor.toml`**

Fill `subpath` and `licence` by inspecting each upstream — do not guess. For every source, run `rtk git ls-remote <upstream>` to confirm the branch name, and read the repository's licence file to get the SPDX id. The plugin caches under `~/.claude/plugins/marketplaces/<name>/` show each project's real directory layout and are the fastest way to find the right `subpath`.

```toml
# Third-party skills vendored into skills/. Run `task skills:sync` to refresh,
# `task skills:sync:check` to see drift without writing.

[sources.superpowers]
upstream = "https://github.com/anthropics/claude-plugins-official"
ref      = "main"
subpath  = "superpowers/skills"     # verify against the cache before committing
provides = "*"
licence  = "MIT"                    # verify against the repository's LICENSE

[sources.ponytail]
upstream = "https://github.com/DietrichGebert/ponytail"
ref      = "main"
subpath  = "plugin/skills"          # verify
provides = "*"
licence  = "MIT"                    # verify

[sources.impeccable]
upstream = "https://github.com/pbakaus/impeccable"
ref      = "main"
subpath  = "plugin/skills"          # verify
provides = "*"
licence  = "MIT"                    # verify

# Installer-based, not vendorable: each writes project-coupled state.
[installers.bmad]
command = "npx bmad-method install"
scope   = "project"

[installers.graphify]
command = "graphify install-skill ."
scope   = "project"
```

Confirm the `graphify` command against `graphify --help` before committing; substitute the real subcommand if it differs.

- [ ] **Step 4: Vendor the skills**

```bash
rtk task skills:sync
```

Expected: every source clones, each skill lands under `skills/<name>/`, and `vendor.toml` gains a `commit` and a `hashes` table per source. Then copy each upstream licence next to its skills and confirm:

```bash
rtk task skills:sync:check
```

Expected: no `MissingLicence` outcomes.

- [ ] **Step 5: Write `profiles.toml`**

Use the real vendored names from `rtk task skills:list`:

```toml
# Named sets of items. Scope is a CLI argument, never recorded here.

[profiles.base]
items = ["ponytail", "graphify"]

[profiles.rust-backend]
include = ["base"]
items = ["bmad"]

[profiles.frontend]
include = ["base"]
items = ["impeccable"]

[profiles.planning]
include = ["base"]
items = ["bmad"]
```

- [ ] **Step 6: Verify a real install and commit the vendored content**

```bash
rtk task skills:install PROFILE=all SCOPE=project
rtk task skills:status
```

Expected: every item reports `Linked`, except installers reporting `InstallerUnmanaged`. Then:

```bash
cat > /tmp/wska-commit.txt <<'MSG'
feat: vendor third-party skills and define profiles

Replaces the abandoned weebo-skills marketplace as the source of truth.
monofolio-design and batlehub are dropped rather than carried over.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add skills profiles.toml vendor.toml && rtk git commit -s -F /tmp/wska-commit.txt'
```

- [ ] **Step 7: Retire the plugin declarations, as its own commit**

Only after step 6 is committed and `skills:status` is clean. Edit `.claude/settings.json`: delete the whole `extraKnownMarketplaces` object and every key of `enabledPlugins`, leaving `hooks` exactly as it is. The result:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash|Grep",
        "hooks": [
          {
            "type": "command",
            "command": "graphify hook-guard search"
          }
        ]
      },
      {
        "matcher": "Read|Glob",
        "hooks": [
          {
            "type": "command",
            "command": "graphify hook-guard read"
          }
        ]
      }
    ]
  }
}
```

Verify the file still parses: `rtk jq . .claude/settings.json`.

```bash
cat > /tmp/wska-commit.txt <<'MSG'
chore: drop plugin marketplaces now that skills are vendored

The plugin path and the wska link path must not both be live for the same
skill, or the same skill exists twice at different versions.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_014tcVTtb2RkbyEU9cocHu3k
MSG
echo 'rtk git add .claude/settings.json && rtk git commit -s -F /tmp/wska-commit.txt'
```

- [ ] **Step 8: Confirm the end state in a fresh session**

Ask the author to restart Claude Code and run `/` to confirm the vendored skills still appear — now reached through `wska`'s symlinks rather than the plugin loader. This is the only check that the whole migration worked, and it cannot be automated from inside a running session.

---

## Self-Review

**Spec coverage.** Symlink distribution → Tasks 7–9. Vendoring → Tasks 11, 13. Rust CLI justification → the whole plan. Repo layout → Tasks 4, 13. `profiles.toml` → Tasks 3, 5, 13. `vendor.toml` incl. the pinned hash → Tasks 2, 3, 11, 13. CLI surface → Tasks 1, 9, 10, 11. Targets → Task 6. Declarative install → Tasks 8, 9. Plan-validate-apply → Tasks 8, 9. Ownership table → Task 7 (one test per row). Installers → Tasks 4, 8, 13. `status` → Task 10. Sync algorithm → Task 11. Licence assertion → Tasks 11, 12. Testing section → every task. Task wiring → Task 12. Migration steps 1–6 → Task 13. No spec section is unimplemented.

**Placeholders.** None: every code step carries the actual code, and the two places needing real-world values (`subpath`, `licence` in Task 13 step 3) carry explicit verification commands rather than "fill in later".

**Type consistency.** `Scope` is defined once in `cli.rs` and reused by `config`, `item`, `plan`, `target`, `app`. `Kind::subdir()` is the single source of the `skills`/`agents`/`mcp` names, used by `item`, `plan` and `status`. `classify(entry, repo_root, expected)` keeps the same signature in Tasks 7, 8 and 10. `hash_dir` returns the `"sha256:<hex>"` form consumed by `sync`. `Provides::Star`/`Named` is used consistently after the Task 3 rename note.

**Review Focus coverage.** All five have a named test in the owning task: relative links (Task 7), symlinked target dir (Task 6), convergence after a partial apply (Task 9), reserved `all` (Task 5), symlink inside a vendored skill (Task 2).
