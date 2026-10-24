use crate::item::Item;
use crate::ownership::{classify, Entry};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum State {
    Linked,
    WrongTarget { points_to: PathBuf },
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
    owned: &[PathBuf],
) -> io::Result<Vec<Row>> {
    let mut rows = Vec::new();
    for (name, item) in items {
        let state = match item {
            Item::Installer { .. } => State::InstallerUnmanaged,
            Item::Linked { kind, path, .. } => {
                let at = target.join(kind.subdir()).join(name);
                match classify(&at, owned, Some(path))? {
                    Entry::OwnedCorrect => State::Linked,
                    Entry::OwnedWrongTarget { points_to } => State::WrongTarget { points_to },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InstallerScope;
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
        for s in [
            "ok",
            "dangly",
            "alien",
            "handmade",
            "absent",
            "mismatched",
            "other",
        ] {
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
        // Points at a different item in this repo, not the expected one.
        symlink(repo.join("skills/other"), target.join("skills/mismatched")).unwrap();

        let mut items: BTreeMap<String, Item> =
            ["ok", "dangly", "alien", "handmade", "absent", "mismatched"]
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
                update: None,
                scope: InstallerScope::Project,
                gemini: None,
                antigravity: None,
                root: repo.clone(),
            },
        );

        let rows = collect(&items, &target, std::slice::from_ref(&repo)).unwrap();
        assert_eq!(state_of(&rows, "ok"), "Linked");
        assert!(state_of(&rows, "dangly").starts_with("Dangling"));
        assert!(state_of(&rows, "alien").starts_with("Foreign"));
        assert_eq!(state_of(&rows, "handmade"), "RealPath");
        assert_eq!(state_of(&rows, "absent"), "MissingFromTarget");
        assert!(state_of(&rows, "mismatched").starts_with("WrongTarget"));
        assert_eq!(state_of(&rows, "bmad"), "InstallerUnmanaged");
    }
}
