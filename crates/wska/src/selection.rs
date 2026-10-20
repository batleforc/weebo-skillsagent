use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What a target asked for: written by every applied `install`, read back
/// when `install` or `update` is given neither `--profile` nor `--item`.
pub const FILE: &str = "wska.toml";

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub items: Vec<String>,
    /// Extra items repos (git URLs) whose items and profiles join this
    /// repo's. Kept by an explicit `install`; only `remove --repo` drops one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("nothing to install: pass --profile or --item, or record a selection in {0}")]
    Missing(PathBuf),
    #[error("{path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty() && self.items.is_empty()
    }

    /// `self` plus whatever of `other` it does not already name, order kept.
    pub fn merged(&self, other: &Self) -> Self {
        let union = |a: &[String], b: &[String]| {
            let mut out = a.to_vec();
            out.extend(b.iter().filter(|n| !a.contains(n)).cloned());
            out
        };
        Self {
            profiles: union(&self.profiles, &other.profiles),
            items: union(&self.items, &other.items),
            repos: union(&self.repos, &other.repos),
        }
    }

    /// The selection recorded in `target_dir`, or `None` when there is none.
    pub fn load(target_dir: &Path) -> Result<Option<Self>, SelectionError> {
        let path = target_dir.join(FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(SelectionError::Io { path, source }),
        };
        toml::from_str(&text)
            .map(Some)
            .map_err(|source| SelectionError::Parse { path, source })
    }

    #[expect(
        clippy::expect_used,
        reason = "Vec<String> fields always serialize; test `round_trips_and_reads_absent_as_none` covers it"
    )]
    pub fn save(&self, target_dir: &Path) -> Result<(), SelectionError> {
        let path = target_dir.join(FILE);
        let body = toml::to_string(self).expect("a list of strings always serializes");
        let text = format!(
            "# Written by `wska install`. Run `wska install` or `wska update` with no\n\
             # --profile/--item to re-apply it; commit it so other machines can.\n{body}"
        );
        std::fs::create_dir_all(target_dir)
            .and_then(|()| std::fs::write(&path, text))
            .map_err(|source| SelectionError::Io { path, source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_reads_absent_as_none() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(Selection::load(d.path()).unwrap(), None);
        let s = Selection {
            profiles: vec!["base".into()],
            items: vec!["weebo-techdoc".into()],
            repos: vec!["https://example.invalid/team-skills".into()],
        };
        s.save(d.path()).unwrap();
        assert_eq!(Selection::load(d.path()).unwrap(), Some(s));
    }
}
