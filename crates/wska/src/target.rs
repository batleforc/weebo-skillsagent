use crate::cli::Scope;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("cannot determine the user config directory: set CLAUDE_CONFIG_DIR or HOME")]
    NoUserDir,
    #[error("cannot pick where to clone the items repo: set WSKA_REPO, XDG_DATA_HOME or HOME")]
    NoCloneDir,
    #[error("`git clone {url} {dir}` failed{detail}")]
    Clone {
        url: String,
        dir: PathBuf,
        detail: String,
    },
    #[error("no profiles.toml in {0}: not a weebo-skillsagent repo")]
    NotItemsRepo(PathBuf),
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

/// Where items come from. `$WSKA_REPO` if set; else the repo containing
/// `cwd` when it holds a `profiles.toml` (working inside this repo); else a
/// clone of `url` that wska owns under `$XDG_DATA_HOME/wska` (or
/// `~/.local/share/wska`). A `$WSKA_REPO` or owned clone that does not exist
/// yet is cloned first, so a fresh machine needs only the binary.
pub fn items_root(
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
    url: &str,
) -> Result<PathBuf, TargetError> {
    let dir = match env("WSKA_REPO") {
        Some(dir) => PathBuf::from(dir),
        None => {
            let here = git_root(cwd)?;
            if here.join("profiles.toml").exists() {
                return Ok(here);
            }
            let data = match env("XDG_DATA_HOME") {
                Some(d) => PathBuf::from(d),
                None => {
                    PathBuf::from(env("HOME").ok_or(TargetError::NoCloneDir)?).join(".local/share")
                }
            };
            data.join("wska/weebo-skillsagent")
        }
    };
    if !dir.exists() {
        clone(url, &dir)?;
    }
    let root = git_root(&dir)?;
    if !root.join("profiles.toml").exists() {
        return Err(TargetError::NotItemsRepo(root));
    }
    Ok(root)
}

fn clone(url: &str, dir: &Path) -> Result<(), TargetError> {
    let fail = |detail: String| TargetError::Clone {
        url: url.to_string(),
        dir: dir.to_path_buf(),
        detail,
    };
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| fail(format!(": {e}")))?;
    }
    eprintln!("cloning {url} into {}", dir.display());
    let status = std::process::Command::new("git")
        .args(["clone", "--quiet", url])
        .arg(dir)
        .status()
        .map_err(|e| fail(format!(": {e}")))?;
    if !status.success() {
        return Err(fail(format!(" ({status})")));
    }
    Ok(())
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

    fn upstream_repo(dir: &Path) -> String {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("profiles.toml"), "").unwrap();
        for args in [
            &["init", "-q"][..],
            &["add", "."],
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "init",
            ],
        ] {
            let ok = std::process::Command::new("git")
                .args(args)
                .current_dir(dir)
                .status()
                .unwrap()
                .success();
            assert!(ok);
        }
        dir.to_str().unwrap().to_string()
    }

    #[test]
    fn items_root_clones_into_the_data_dir_once_then_reuses_it() {
        let d = tempfile::tempdir().unwrap();
        let url = upstream_repo(&d.path().join("upstream"));
        let project = d.path().join("project");
        fs::create_dir_all(project.join(".git")).unwrap();
        let data = d.path().join("data");
        let env = env_from(&[("XDG_DATA_HOME", data.to_str().unwrap())]);

        let got = items_root(&project, &env, &url).unwrap();
        assert_eq!(
            got,
            data.join("wska/weebo-skillsagent").canonicalize().unwrap()
        );
        // Second run must not re-clone (a clone into a non-empty dir would fail).
        assert_eq!(items_root(&project, &env, &url).unwrap(), got);
    }

    #[test]
    fn items_root_prefers_the_current_repo_when_it_has_profiles() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join(".git")).unwrap();
        fs::write(d.path().join("profiles.toml"), "").unwrap();
        let env = env_from(&[]);
        let got = items_root(d.path(), &env, "unused").unwrap();
        assert_eq!(got, d.path().canonicalize().unwrap());
    }

    #[test]
    fn items_root_rejects_a_wska_repo_without_profiles() {
        let d = tempfile::tempdir().unwrap();
        let env = env_from(&[("WSKA_REPO", d.path().to_str().unwrap())]);
        let result = items_root(d.path(), &env, "unused");
        assert!(matches!(result, Err(TargetError::NotItemsRepo(_))));
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
