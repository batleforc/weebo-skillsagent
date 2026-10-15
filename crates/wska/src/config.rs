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
        assert!(matches!(s.provides, Provides::Star(ref v) if v == "*"));
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

    #[test]
    fn missing_files_load_as_empty() {
        let d = tempfile::tempdir().unwrap();
        let missing = d.path().join("nope.toml");
        assert!(load_profiles(&missing).unwrap().profiles.is_empty());
        assert!(load_vendor(&missing).unwrap().sources.is_empty());
    }
}
