use crate::cli::{Agent, Scope};
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
        owned: Vec<PathBuf>,
    },
    RunInstaller {
        name: String,
        command: String,
        cwd: PathBuf,
        /// The installer's own repo, exported as `WSKA_REPO` so its command
        /// can reference files there (`uv tool install "$WSKA_REPO/judge"`).
        repo_root: PathBuf,
    },
    /// An installer not run, or an item the agent has no place for.
    Skip {
        name: String,
        reason: String,
    },
}

/// What to do with installers: leave them, run their install command, or
/// their update command (falling back to install when none is declared).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installers {
    Skip,
    Install,
    Update,
}

impl Installers {
    /// `--run-installers` as passed to install/add/remove.
    pub fn install_if(run: bool) -> Self {
        if run {
            Installers::Install
        } else {
            Installers::Skip
        }
    }
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
    owned: &[PathBuf],
    scope: Scope,
    agent: Agent,
    installers: Installers,
) -> io::Result<Plan> {
    let mut plan = Plan::default();

    // Wanted items: link what is missing, refuse what we do not own.
    for name in wanted {
        let Some(item) = items.get(name) else {
            continue;
        };
        match item {
            // Other agents' subagent and MCP formats differ from Claude Code's.
            Item::Linked { kind, .. } if agent != Agent::Claude && *kind != Kind::Skill => {
                plan.actions.push(Action::Skip {
                    name: name.clone(),
                    reason: format!(
                        "{} are Claude Code only, not linked for {agent:?}",
                        kind.subdir()
                    ),
                });
            }
            Item::Linked { kind, path, .. } => {
                let at = target.join(kind.subdir()).join(name);
                match classify(&at, owned, Some(path))? {
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
                update,
                scope: want,
                gemini,
                antigravity,
                root,
                ..
            } => {
                let setup = match agent {
                    Agent::Claude => Some((command, update, *want)),
                    Agent::Gemini => gemini.as_ref().map(|s| (&s.command, &s.update, s.scope)),
                    Agent::Antigravity => antigravity
                        .as_ref()
                        .map(|s| (&s.command, &s.update, s.scope)),
                };
                let Some((command, update, want)) = setup else {
                    plan.actions.push(Action::Skip {
                        name: name.clone(),
                        reason: format!(
                            "vendor.toml declares no {agent:?} setup, Claude Code only"
                        ),
                    });
                    continue;
                };
                if !want.admits(scope) {
                    plan.actions.push(Action::Skip {
                        name: name.clone(),
                        reason: format!("declared for {want:?} scope, invoked at {scope:?}"),
                    });
                } else if installers != Installers::Skip {
                    let command = match (installers, update) {
                        (Installers::Update, Some(update)) => update,
                        _ => command,
                    };
                    plan.actions.push(Action::RunInstaller {
                        name: name.clone(),
                        command: command.clone(),
                        cwd: target.parent().unwrap_or(target).to_path_buf(),
                        repo_root: root.clone(),
                    });
                } else {
                    plan.actions.push(Action::Skip {
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
            match classify(&entry.path(), owned, None)? {
                Entry::OwnedCorrect
                | Entry::OwnedWrongTarget { .. }
                | Entry::OwnedDangling { .. } => {
                    plan.actions.push(Action::Unlink {
                        at: entry.path(),
                        owned: owned.to_vec(),
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
    use crate::config::InstallerScope;
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
            std::slice::from_ref(&f.repo),
            Scope::Project,
            Agent::Claude,
            Installers::Skip,
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
            std::slice::from_ref(&f.repo),
            Scope::Project,
            Agent::Claude,
            Installers::Skip,
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
            std::slice::from_ref(&f.repo),
            Scope::Project,
            Agent::Claude,
            Installers::Skip,
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
            std::slice::from_ref(&f.repo),
            Scope::Project,
            Agent::Claude,
            Installers::Skip,
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
            std::slice::from_ref(&f.repo),
            Scope::Project,
            Agent::Claude,
            Installers::Skip,
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
            std::slice::from_ref(&f.repo),
            Scope::Project,
            Agent::Claude,
            Installers::Skip,
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
            std::slice::from_ref(&f.repo),
            Scope::Project,
            Agent::Claude,
            Installers::Skip,
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
            std::slice::from_ref(&f.repo),
            Scope::Project,
            Agent::Claude,
            Installers::Skip,
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
                update: None,
                scope: InstallerScope::Project,
                gemini: Some(crate::config::Setup {
                    command: "echo gemini".into(),
                    update: None,
                    scope: InstallerScope::User,
                }),
                antigravity: None,
                root: PathBuf::new(),
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
            std::slice::from_ref(&f.repo),
            Scope::Project,
            Agent::Claude,
            Installers::Skip,
        )
        .unwrap();
        assert!(matches!(plan.actions[0], Action::Skip { .. }));
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn runs_a_project_installer_at_project_scope_with_the_flag() {
        let f = fx(&[]);
        let plan = build(
            &bmad_map(),
            &set(&["bmad"]),
            &f.target,
            std::slice::from_ref(&f.repo),
            Scope::Project,
            Agent::Claude,
            Installers::Install,
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
            std::slice::from_ref(&f.repo),
            Scope::User,
            Agent::Claude,
            Installers::Install,
        )
        .unwrap();
        assert!(matches!(plan.actions[0], Action::Skip { .. }));
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn gemini_links_skills_and_skips_agents_and_installers() {
        let f = fx(&["ponytail"]);
        let mut map = bmad_map();
        map.extend(items(&f.repo, &["ponytail"]));
        map.insert(
            "doc-review".to_string(),
            Item::Linked {
                name: "doc-review".into(),
                kind: Kind::Agent,
                path: f.repo.join("agents/doc-review"),
            },
        );
        let plan = build(
            &map,
            &set(&["bmad", "doc-review", "ponytail"]),
            &f.target,
            std::slice::from_ref(&f.repo),
            Scope::Project,
            Agent::Gemini,
            Installers::Install,
        )
        .unwrap();
        let skipped: Vec<&str> = plan
            .actions
            .iter()
            .filter_map(|a| match a {
                Action::Skip { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(skipped, ["bmad", "doc-review"]);
        assert!(plan
            .actions
            .iter()
            .any(|a| matches!(a, Action::Link { to, .. } if to.ends_with("skills/ponytail"))));
    }

    #[test]
    fn an_installer_runs_its_own_command_and_scope_per_agent() {
        let f = fx(&[]);
        let run = |scope, agent| {
            let plan = build(
                &bmad_map(),
                &set(&["bmad"]),
                &f.target,
                std::slice::from_ref(&f.repo),
                scope,
                agent,
                Installers::Install,
            )
            .unwrap();
            match &plan.actions[0] {
                Action::RunInstaller { command, .. } => Some(command.clone()),
                _ => None,
            }
        };
        assert_eq!(
            run(Scope::User, Agent::Gemini).as_deref(),
            Some("echo gemini")
        );
        assert_eq!(
            run(Scope::Project, Agent::Gemini),
            None,
            "gemini setup is user-scoped"
        );
        assert_eq!(
            run(Scope::Project, Agent::Antigravity),
            None,
            "no antigravity setup"
        );
    }

    #[test]
    fn update_runs_the_update_command_and_falls_back_to_install() {
        let f = fx(&[]);
        let run = |update: Option<&str>, installers| {
            let mut map = bmad_map();
            if let Some(Item::Installer { update: u, .. }) = map.get_mut("bmad") {
                *u = update.map(str::to_string);
            }
            let plan = build(
                &map,
                &set(&["bmad"]),
                &f.target,
                std::slice::from_ref(&f.repo),
                Scope::Project,
                Agent::Claude,
                installers,
            )
            .unwrap();
            match &plan.actions[0] {
                Action::RunInstaller { command, .. } => command.clone(),
                other => panic!("expected RunInstaller, got {other:?}"),
            }
        };
        assert_eq!(run(Some("echo up"), Installers::Update), "echo up");
        assert_eq!(run(None, Installers::Update), "echo hi");
        assert_eq!(run(Some("echo up"), Installers::Install), "echo hi");
    }

    #[test]
    fn an_any_scope_installer_runs_at_both_scopes() {
        let f = fx(&[]);
        let mut map = BTreeMap::new();
        map.insert(
            "judge".to_string(),
            Item::Installer {
                name: "judge".into(),
                command: "true".into(),
                update: None,
                scope: InstallerScope::Any,
                gemini: None,
                antigravity: None,
                root: f.repo.clone(),
            },
        );
        for scope in [Scope::Project, Scope::User] {
            let plan = build(
                &map,
                &set(&["judge"]),
                &f.target,
                std::slice::from_ref(&f.repo),
                scope,
                Agent::Claude,
                Installers::Install,
            )
            .unwrap();
            match &plan.actions[0] {
                Action::RunInstaller { repo_root, .. } => assert_eq!(repo_root, &f.repo),
                other => panic!("expected RunInstaller at {scope:?}, got {other:?}"),
            }
        }
    }
}
