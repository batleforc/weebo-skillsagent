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
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("item name `{name}` is claimed twice: {first} and {second}")]
    DuplicateName {
        name: String,
        first: String,
        second: String,
    },
    #[error("vendor.toml [requires] names `{name}`, which is not an item in this repo")]
    UnknownRequirement { name: String },
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
            let md = match std::fs::metadata(entry.path()) {
                Ok(md) => md,
                // A dangling entry is junk, not a reason to fail the whole scan.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(ScanError::Io {
                        path: entry.path(),
                        source,
                    })
                }
            };
            if !md.is_dir() {
                continue;
            }
            insert(
                &mut out,
                Item::Linked {
                    name: name.clone(),
                    kind,
                    path: entry.path(),
                },
                kind.subdir(),
            )?;
        }
    }

    for (name, installer) in &vendor.installers {
        insert(
            &mut out,
            Item::Installer {
                name: name.clone(),
                command: installer.command.clone(),
                scope: installer.scope,
            },
            "installers",
        )?;
    }

    // A [requires] key that names nothing would silently disable that item's
    // health check, so a typo must fail loudly rather than report Ok.
    for name in vendor.requires.keys() {
        if !out.contains_key(name) {
            return Err(ScanError::UnknownRequirement { name: name.clone() });
        }
    }

    Ok(out)
}

fn insert(out: &mut BTreeMap<String, Item>, item: Item, origin: &str) -> Result<(), ScanError> {
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
            Installer {
                command: "echo hi".into(),
                scope,
            },
        );
        v
    }

    #[test]
    fn rejects_a_requires_key_that_names_no_item() {
        let d = repo(&[("skills", "real")]);
        let mut v = Vendor::default();
        v.requires
            .insert("reel".to_string(), vec!["npx".to_string()]);
        let err = scan(d.path(), &v).unwrap_err();
        assert!(
            matches!(&err, ScanError::UnknownRequirement { name } if name == "reel"),
            "got {err:?}"
        );
    }

    #[test]
    fn accepts_a_requires_key_for_an_installer() {
        let d = repo(&[]);
        let mut v = vendor_with_installer("bmad", Scope::Project);
        v.requires
            .insert("bmad".to_string(), vec!["npx".to_string()]);
        assert!(scan(d.path(), &v).is_ok());
    }

    #[test]
    fn finds_skills_and_agents() {
        let d = repo(&[("skills", "ponytail"), ("agents", "committer")]);
        let got = scan(d.path(), &Vendor::default()).unwrap();
        assert!(matches!(
            got["ponytail"],
            Item::Linked {
                kind: Kind::Skill,
                ..
            }
        ));
        assert!(matches!(
            got["committer"],
            Item::Linked {
                kind: Kind::Agent,
                ..
            }
        ));
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
        match scan(d.path(), &Vendor::default()).unwrap_err() {
            ScanError::DuplicateName { name, .. } => assert_eq!(name, "clash"),
            other => panic!("expected DuplicateName, got {other:?}"),
        }
    }

    #[test]
    fn rejects_an_installer_shadowing_a_skill() {
        let d = repo(&[("skills", "bmad")]);
        match scan(d.path(), &vendor_with_installer("bmad", Scope::Project)).unwrap_err() {
            ScanError::DuplicateName { name, .. } => assert_eq!(name, "bmad"),
            other => panic!("expected DuplicateName, got {other:?}"),
        }
    }

    #[test]
    fn ignores_files_and_dotdirs_at_the_kind_level() {
        let d = repo(&[("skills", "ponytail")]);
        fs::write(d.path().join("skills/README.md"), "x").unwrap();
        fs::create_dir(d.path().join("skills/.cache")).unwrap();
        let got = scan(d.path(), &Vendor::default()).unwrap();
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn symlink_to_directory_is_included() {
        use std::os::unix::fs::symlink;

        let d = repo(&[("skills", "ponytail")]);
        let target = d.path().join("target_dir");
        fs::create_dir(&target).unwrap();
        symlink(&target, d.path().join("skills/linked")).unwrap();
        let got = scan(d.path(), &Vendor::default()).unwrap();
        assert!(matches!(
            got["linked"],
            Item::Linked {
                kind: Kind::Skill,
                ..
            }
        ));
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn dangling_symlink_is_skipped() {
        use std::os::unix::fs::symlink;

        let d = repo(&[("skills", "ponytail")]);
        symlink("/nonexistent/target", d.path().join("skills/dangling")).unwrap();
        let got = scan(d.path(), &Vendor::default()).unwrap();
        assert!(!got.contains_key("dangling"));
        assert_eq!(got.len(), 1);
    }
}
