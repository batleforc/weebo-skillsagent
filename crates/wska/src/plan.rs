use crate::cli::Scope;
use crate::item::{Item, Kind};
use crate::ownership::{classify, Entry};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum Action {
    Link {
        from: PathBuf,
        to: PathBuf,
    },
    Unlink {
        at: PathBuf,
        repo_root: PathBuf,
    },
    RunInstaller {
        name: String,
        command: String,
        cwd: PathBuf,
    },
    SkipInstaller {
        name: String,
        reason: String,
    },
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
    run_installers: bool,
) -> io::Result<Plan> {
    let mut plan = Plan::default();

    // Wanted items: link what is missing, refuse what we do not own.
    for name in wanted {
        let Some(item) = items.get(name) else {
            continue;
        };
        match item {
            Item::Linked { kind, path, .. } => {
                let at = target.join(kind.subdir()).join(name);
                match classify(&at, repo_root, Some(path))? {
                    Entry::OwnedCorrect => {}
                    Entry::Absent
                    | Entry::OwnedWrongTarget { .. }
                    | Entry::OwnedDangling { .. } => {
                        plan.actions.push(Action::Link {
                            from: path.clone(),
                            to: at,
                        });
                    }
                    Entry::Foreign { points_to } => plan.refusals.push(Refusal {
                        path: at,
                        reason: format!(
                            "symlink points outside this repo, at {}",
                            points_to.display()
                        ),
                    }),
                    Entry::RealPath => plan.refusals.push(Refusal {
                        path: at,
                        reason: "a real file or directory is already there".to_string(),
                    }),
                }
            }
            Item::Installer {
                command,
                scope: want,
                ..
            } => {
                if *want != scope {
                    plan.actions.push(Action::SkipInstaller {
                        name: name.clone(),
                        reason: format!("declared for {want:?} scope, invoked at {scope:?}"),
                    });
                } else if run_installers {
                    plan.actions.push(Action::RunInstaller {
                        name: name.clone(),
                        command: command.clone(),
                        cwd: target.parent().unwrap_or(target).to_path_buf(),
                    });
                } else {
                    plan.actions.push(Action::SkipInstaller {
                        name: name.clone(),
                        reason: "installers run only with --run-installers".to_string(),
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
            let keep = wanted.contains(&name)
                && matches!(
                    items.get(&name),
                    Some(Item::Linked { kind: item_kind, .. }) if *item_kind == kind
                );
            if keep {
                continue;
            }
            match classify(&entry.path(), repo_root, None)? {
                Entry::OwnedCorrect
                | Entry::OwnedWrongTarget { .. }
                | Entry::OwnedDangling { .. } => {
                    plan.actions.push(Action::Unlink {
                        at: entry.path(),
                        repo_root: repo_root.to_path_buf(),
                    });
                }
                _ => {}
            }
        }
    }

    Ok(plan)
}

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
        fs::create_dir_all(&repo).unwrap();
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
            false,
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
            false,
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
            false,
        )
        .unwrap();
        assert!(plan
            .actions
            .iter()
            .any(|a| matches!(a, Action::Unlink { .. })));
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
            false,
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
            false,
        )
        .unwrap();
        assert_eq!(plan.refusals.len(), 1);
        assert!(plan.validate().is_err());
    }

    #[test]
    fn leaves_an_unwanted_real_directory_alone_without_refusing() {
        let f = fx(&["ponytail"]);
        fs::create_dir(f.target.join("skills/handmade")).unwrap();
        let plan = build(
            &items(&f.repo, &["ponytail"]),
            &set(&["ponytail"]),
            &f.target,
            &f.repo,
            Scope::Project,
            false,
        )
        .unwrap();
        assert!(plan.refusals.is_empty(), "{:?}", plan.refusals);
        assert!(!plan
            .actions
            .iter()
            .any(|a| matches!(a, Action::Unlink { at, .. } if at.ends_with("handmade"))));
    }

    #[test]
    fn leaves_an_unwanted_foreign_symlink_alone_without_refusing() {
        let f = fx(&["ponytail"]);
        let outside = f.repo.parent().unwrap().join("elsewhere");
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, f.target.join("skills/handmade")).unwrap();
        let plan = build(
            &items(&f.repo, &["ponytail"]),
            &set(&["ponytail"]),
            &f.target,
            &f.repo,
            Scope::Project,
            false,
        )
        .unwrap();
        assert!(plan.refusals.is_empty(), "{:?}", plan.refusals);
        assert!(!plan
            .actions
            .iter()
            .any(|a| matches!(a, Action::Unlink { at, .. } if at.ends_with("handmade"))));
    }

    #[test]
    fn converges_when_an_item_moves_to_a_different_kind() {
        let f = fx(&[]);
        let agent_path = f.repo.join("agents").join("ponytail");
        fs::create_dir_all(&agent_path).unwrap();
        // Stale link left over from when `ponytail` lived under skills/.
        symlink(
            f.repo.join("skills/ponytail"),
            f.target.join("skills/ponytail"),
        )
        .unwrap();

        let mut map = BTreeMap::new();
        map.insert(
            "ponytail".to_string(),
            Item::Linked {
                name: "ponytail".to_string(),
                kind: Kind::Agent,
                path: agent_path,
            },
        );

        let plan = build(
            &map,
            &set(&["ponytail"]),
            &f.target,
            &f.repo,
            Scope::Project,
            false,
        )
        .unwrap();

        assert!(
            plan.actions
                .iter()
                .any(|a| matches!(a, Action::Link { to, .. } if to.ends_with("agents/ponytail"))),
            "{:?}",
            plan.actions
        );
        assert!(
            plan.actions
                .iter()
                .any(|a| matches!(a, Action::Unlink { at, .. } if at.ends_with("skills/ponytail"))),
            "{:?}",
            plan.actions
        );
    }

    fn bmad_map() -> BTreeMap<String, Item> {
        let mut map = BTreeMap::new();
        map.insert(
            "bmad".to_string(),
            Item::Installer {
                name: "bmad".into(),
                command: "echo hi".into(),
                scope: Scope::Project,
            },
        );
        map
    }

    #[test]
    fn skips_a_matching_installer_without_the_run_installers_flag() {
        let f = fx(&[]);
        let plan = build(
            &bmad_map(),
            &set(&["bmad"]),
            &f.target,
            &f.repo,
            Scope::Project,
            false,
        )
        .unwrap();
        assert!(matches!(plan.actions[0], Action::SkipInstaller { .. }));
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn runs_a_project_installer_at_project_scope_with_the_flag() {
        let f = fx(&[]);
        let plan = build(
            &bmad_map(),
            &set(&["bmad"]),
            &f.target,
            &f.repo,
            Scope::Project,
            true,
        )
        .unwrap();
        assert!(matches!(plan.actions[0], Action::RunInstaller { .. }));
    }

    #[test]
    fn skips_a_project_installer_at_user_scope_even_with_the_flag() {
        let f = fx(&[]);
        let plan = build(
            &bmad_map(),
            &set(&["bmad"]),
            &f.target,
            &f.repo,
            Scope::User,
            true,
        )
        .unwrap();
        assert!(matches!(plan.actions[0], Action::SkipInstaller { .. }));
        assert!(plan.validate().is_ok());
    }
}
