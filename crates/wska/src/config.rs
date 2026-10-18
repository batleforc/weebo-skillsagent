use crate::cli::Scope;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("reading {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parsing {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("source `{source_name}`: provides must be \"*\" or a list of names, got \"{got}\"")]
    Provides { source_name: String, got: String },
    #[error("source `{source_name}`: copies path `{path}` must be relative with no `..`")]
    CopyPath { source_name: String, path: String },
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
    /// External commands each item needs, keyed by item name. An entry may
    /// list alternatives separated by `|` ("google-chrome|chromium"), which
    /// is satisfied when any one of them resolves.
    #[serde(default)]
    pub requires: BTreeMap<String, Vec<String>>,
    /// Skills fetched from a git repo at install time rather than vendored.
    #[serde(default)]
    pub external: BTreeMap<String, External>,
}

#[derive(Debug, Deserialize)]
pub struct Source {
    pub upstream: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    #[serde(default)]
    pub subpath: String,
    #[serde(default)]
    pub provides: Provides,
    /// Directories copied verbatim from an upstream that is not laid out as
    /// skills: local path (from the repo root) -> upstream path (from the
    /// clone root). Each is hashed and fast-forwarded like a provided skill,
    /// keyed by its local path.
    #[serde(default)]
    pub copies: BTreeMap<String, String>,
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

impl Default for Provides {
    fn default() -> Self {
        Provides::Named(Vec::new())
    }
}

#[derive(Debug, Deserialize)]
pub struct External {
    pub upstream: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    /// The skill directory inside the clone.
    pub path: String,
    pub licence: Option<String>,
}

/// Where an installer runs. `Any` runs at whichever scope install is invoked
/// with: a machine-wide tool (judge, via `uv tool`) that a project-scoped
/// item needs would otherwise be skipped by the default `--scope project`.
/// Kept apart from `cli::Scope` so `--scope any` never becomes a CLI value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallerScope {
    Project,
    User,
    Any,
}

impl InstallerScope {
    pub fn admits(self, scope: Scope) -> bool {
        match self {
            InstallerScope::Any => true,
            InstallerScope::Project => scope == Scope::Project,
            InstallerScope::User => scope == Scope::User,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Installer {
    pub command: String,
    pub scope: InstallerScope,
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
        // vendor.toml is ours, but a copy outside the repo would be written by sync.
        for path in source.copies.iter().flat_map(|(local, up)| [local, up]) {
            let p = Path::new(path);
            if p.is_absolute()
                || p.components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(ConfigError::CopyPath {
                    source_name: name.clone(),
                    path: path.clone(),
                });
            }
        }
    }
    Ok(vendor)
}

fn parse<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T, ConfigError> {
    let body = match std::fs::read_to_string(path) {
        Ok(body) => body,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(source) => {
            return Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    toml::from_str(&body).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

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
        let (_d, p) = tmp(r#"
[profiles.base]
items = ["ponytail"]

[profiles.frontend]
include = ["base"]
items = ["impeccable"]
"#);
        let got = load_profiles(&p).unwrap();
        assert_eq!(got.profiles["base"].include, Vec::<String>::new());
        assert_eq!(got.profiles["frontend"].include, vec!["base".to_string()]);
        assert_eq!(
            got.profiles["frontend"].items,
            vec!["impeccable".to_string()]
        );
    }

    #[test]
    fn parses_vendor_source_with_star_provides() {
        let (_d, p) = tmp(r#"
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
"#);
        let got = load_vendor(&p).unwrap();
        let s = &got.sources["superpowers"];
        assert_eq!(s.git_ref, "main");
        assert!(matches!(s.provides, Provides::Star(ref v) if v == "*"));
        assert_eq!(s.hashes["brainstorming"], "sha256:00");
        assert_eq!(got.installers["bmad"].scope, InstallerScope::Project);
    }

    #[test]
    fn parses_vendor_source_with_named_provides() {
        let (_d, p) = tmp(r#"
[sources.two]
upstream = "https://example.invalid/x"
ref = "v1"
subpath = "skills"
provides = ["a", "b"]
licence = "MIT"
"#);
        let got = load_vendor(&p).unwrap();
        match &got.sources["two"].provides {
            Provides::Named(v) => assert_eq!(v, &vec!["a".to_string(), "b".to_string()]),
            other => panic!("expected Named, got {other:?}"),
        }
    }

    #[test]
    fn rejects_provides_string_that_is_not_star() {
        let (_d, p) = tmp(r#"
[sources.bad]
upstream = "https://example.invalid/x"
ref = "main"
subpath = "skills"
provides = "everything"
licence = "MIT"
"#);
        let err = load_vendor(&p).unwrap_err().to_string();
        assert!(err.contains("provides"), "unexpected error: {err}");
    }

    #[test]
    fn parses_a_copies_only_source() {
        let (_d, p) = tmp(r#"
[sources.nr]
upstream = "https://example.invalid/x"
ref = "main"
copies = { "skills/me/references/rules" = ".continue/rules" }
licence = "CC-BY-SA-4.0"
"#);
        let got = load_vendor(&p).unwrap();
        let s = &got.sources["nr"];
        assert!(matches!(&s.provides, Provides::Named(v) if v.is_empty()));
        assert_eq!(s.copies["skills/me/references/rules"], ".continue/rules");
    }

    #[test]
    fn rejects_a_copy_path_leaving_the_repo() {
        for copies in [
            r#"{ "../outside" = "rules" }"#,
            r#"{ "skills/x" = "/etc" }"#,
        ] {
            let (_d, p) = tmp(&format!(
                "[sources.nr]\nupstream = \"x\"\nref = \"main\"\nlicence = \"MIT\"\ncopies = {copies}\n"
            ));
            let err = load_vendor(&p).unwrap_err().to_string();
            assert!(err.contains("copies path"), "{copies}: {err}");
        }
    }

    #[test]
    fn missing_files_load_as_empty() {
        let d = tempfile::tempdir().unwrap();
        let missing = d.path().join("nope.toml");
        assert!(load_profiles(&missing).unwrap().profiles.is_empty());
        assert!(load_vendor(&missing).unwrap().sources.is_empty());
    }

    #[test]
    fn parses_requires_table_keyed_by_item_name() {
        let (_d, p) = tmp(r#"
[requires]
graphify = ["graphify"]
bmad = ["npx", "uv"]
frontend-design = ["google-chrome|chromium"]
"#);
        let got = load_vendor(&p).unwrap();
        assert_eq!(
            got.requires["bmad"],
            vec!["npx".to_string(), "uv".to_string()]
        );
        assert_eq!(got.requires["graphify"], vec!["graphify".to_string()]);
        assert_eq!(
            got.requires["frontend-design"],
            vec!["google-chrome|chromium".to_string()]
        );
    }

    #[test]
    fn parses_external_skill() {
        let (_d, p) = tmp(r#"
[external.rfc]
upstream = "https://example.invalid/x"
ref = "main"
path = ".claude/skills/rfc"
licence = "Apache-2.0"
"#);
        let got = load_vendor(&p).unwrap();
        let e = &got.external["rfc"];
        assert_eq!(e.git_ref, "main");
        assert_eq!(e.path, ".claude/skills/rfc");
    }

    #[test]
    fn requires_table_is_optional() {
        let (_d, p) = tmp("[installers.x]\ncommand = \"true\"\nscope = \"project\"\n");
        assert!(load_vendor(&p).unwrap().requires.is_empty());
    }

    #[test]
    fn parses_an_any_scope_installer() {
        let (_d, p) = tmp("[installers.judge]\ncommand = \"true\"\nscope = \"any\"\n");
        let got = load_vendor(&p).unwrap();
        assert_eq!(got.installers["judge"].scope, InstallerScope::Any);
    }

    #[test]
    fn any_admits_both_scopes_and_the_others_only_their_own() {
        use crate::cli::Scope;
        assert!(InstallerScope::Any.admits(Scope::Project));
        assert!(InstallerScope::Any.admits(Scope::User));
        assert!(InstallerScope::Project.admits(Scope::Project));
        assert!(!InstallerScope::Project.admits(Scope::User));
        assert!(!InstallerScope::User.admits(Scope::Project));
    }
}
