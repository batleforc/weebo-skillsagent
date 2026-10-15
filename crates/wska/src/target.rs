use crate::cli::Scope;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("cannot determine the user config directory: set CLAUDE_CONFIG_DIR or HOME")]
    NoUserDir,
    #[error("canonicalizing {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
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
    canonical_or_as_is(&raw)
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
        let result = resolve(Scope::User, d.path(), &env);
        assert!(matches!(result, Err(TargetError::NoUserDir)));
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
