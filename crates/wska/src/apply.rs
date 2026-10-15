use crate::ownership::{classify, Entry};
use crate::plan::{Action, Plan};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    pub linked: usize,
    pub unlinked: usize,
    pub installers_run: Vec<String>,
    pub skipped: Vec<String>,
    /// Items that were linked but whose declared tools are not installed.
    /// Advisory only: a skill is still readable without its tools.
    pub warnings: Vec<String>,
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
            Action::Unlink { at, repo_root } => {
                println!("unlink {}", at.display());
                if !dry_run {
                    // Re-check ownership here, not just at plan time: the
                    // path may have been replaced by something we do not
                    // own in the window between planning and applying.
                    let owned = matches!(
                        classify(at, repo_root, None).map_err(|source| ApplyError::Io {
                            op: "inspecting",
                            path: at.clone(),
                            source,
                        })?,
                        Entry::OwnedCorrect
                            | Entry::OwnedWrongTarget { .. }
                            | Entry::OwnedDangling { .. }
                    );
                    if owned {
                        remove_if_link(at)?;
                        done.unlinked += 1;
                    } else {
                        let msg = format!("{} changed since planning, left alone", at.display());
                        println!("skip   {msg}");
                        done.skipped.push(msg);
                    }
                } else {
                    done.unlinked += 1;
                }
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
        let repo_root = d.path().join("repo");
        let from = repo_root.join("skills/ponytail");
        fs::create_dir_all(&from).unwrap();
        let to = d.path().join("proj/.claude/skills/ponytail");
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        symlink(&from, &to).unwrap();

        let plan = Plan {
            actions: vec![Action::Unlink {
                at: to.clone(),
                repo_root: repo_root.clone(),
            }],
            refusals: vec![],
        };
        let got = apply(&plan, false).unwrap();
        assert_eq!(got.unlinked, 1);
        assert!(!to.exists());
        assert!(from.exists(), "the item itself must survive");
    }

    // Review Focus 3 / Finding 3: ownership is re-checked right before
    // removal, not trusted from plan time.
    #[test]
    fn unlink_leaves_a_path_that_turned_foreign_since_planning() {
        let d = tempfile::tempdir().unwrap();
        let repo_root = d.path().join("repo");
        let from = repo_root.join("skills/ponytail");
        fs::create_dir_all(&from).unwrap();
        let to = d.path().join("proj/.claude/skills/ponytail");
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        // Planned against an owned link...
        symlink(&from, &to).unwrap();
        let plan = Plan {
            actions: vec![Action::Unlink {
                at: to.clone(),
                repo_root: repo_root.clone(),
            }],
            refusals: vec![],
        };

        // ...but before apply runs, something else replaces it with a
        // symlink pointing outside the repo.
        let outside = d.path().join("elsewhere");
        fs::create_dir_all(&outside).unwrap();
        fs::remove_file(&to).unwrap();
        symlink(&outside, &to).unwrap();

        let got = apply(&plan, false).unwrap();
        assert_eq!(got.unlinked, 0);
        assert_eq!(got.skipped.len(), 1);
        assert!(to.exists(), "the foreign link must survive");
        assert_eq!(fs::read_link(&to).unwrap(), outside);
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
