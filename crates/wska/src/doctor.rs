use crate::item::{Item, Kind};
use std::collections::BTreeMap;
use std::path::Path;

/// Whether an item can actually work here.
#[derive(Debug)]
pub enum Health {
    Ok,
    /// Declared requirements that nothing on PATH satisfies.
    Missing(Vec<String>),
    /// The item is not a usable skill at all, for the stated reason.
    Invalid(String),
}

impl Health {
    pub fn is_ok(&self) -> bool {
        matches!(self, Health::Ok)
    }
}

#[derive(Debug)]
pub struct Row {
    pub name: String,
    pub health: Health,
}

/// One row per item, in name order.
pub fn collect(
    items: &BTreeMap<String, Item>,
    requires: &BTreeMap<String, Vec<String>>,
    has: &dyn Fn(&str) -> bool,
) -> Vec<Row> {
    items
        .iter()
        .map(|(name, item)| Row {
            name: name.clone(),
            health: health(
                item,
                requires.get(name).map(Vec::as_slice).unwrap_or(&[]),
                has,
            ),
        })
        .collect()
}

fn health(item: &Item, requirements: &[String], has: &dyn Fn(&str) -> bool) -> Health {
    // A directory that is not a valid skill cannot work whatever tools are
    // installed, so that outranks a missing requirement. Installers have no
    // SKILL.md of their own to check.
    if let Item::Linked {
        kind: Kind::Skill,
        path,
        ..
    } = item
    {
        if let Err(reason) = validate_skill_md(path) {
            return Health::Invalid(reason);
        }
    }

    let missing = unsatisfied(requirements, has);
    if missing.is_empty() {
        Health::Ok
    } else {
        Health::Missing(missing)
    }
}

/// A requirement is satisfied when any of its `|`-separated alternatives
/// resolves, so `"google-chrome|chromium"` passes on a box with either.
pub fn satisfied(requirement: &str, has: &dyn Fn(&str) -> bool) -> bool {
    requirement
        .split('|')
        .map(str::trim)
        .filter(|tool| !tool.is_empty())
        .any(has)
}

/// The requirements that are not satisfied, in declaration order.
pub fn unsatisfied(requirements: &[String], has: &dyn Fn(&str) -> bool) -> Vec<String> {
    requirements
        .iter()
        .filter(|requirement| !satisfied(requirement, has))
        .cloned()
        .collect()
}

/// Whether `tool` resolves to an executable file on the given PATH value. The
/// PATH is a parameter rather than read from the environment so that tests do
/// not have to mutate process state.
pub fn resolves_in(path_var: &str, tool: &str) -> bool {
    std::env::split_paths(path_var).any(|dir| is_executable(&dir.join(tool)))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|md| md.is_file() && md.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Whether a directory really is a usable skill: a `SKILL.md` whose YAML
/// frontmatter carries a non-empty `name` and `description`. Those two are
/// what the harness reads to list and trigger a skill, so a copy missing
/// either is inert however well it synced.
pub fn validate_skill_md(dir: &Path) -> Result<(), String> {
    let path = dir.join("SKILL.md");
    let body = std::fs::read_to_string(&path).map_err(|e| format!("SKILL.md unreadable: {e}"))?;

    let frontmatter = body
        .strip_prefix("---")
        .and_then(|rest| rest.split_once("\n---"))
        .map(|(front, _)| front)
        .ok_or_else(|| "no YAML frontmatter delimited by ---".to_string())?;

    for field in ["name", "description"] {
        let value = frontmatter.lines().find_map(|line| {
            line.trim()
                .strip_prefix(field)
                .and_then(|rest| rest.strip_prefix(':'))
        });
        match value {
            None => return Err(format!("frontmatter has no `{field}`")),
            Some(v) if v.trim().is_empty() => {
                return Err(format!("frontmatter `{field}` is empty"))
            }
            Some(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn has(present: &[&str]) -> impl Fn(&str) -> bool {
        let owned: Vec<String> = present.iter().map(|s| s.to_string()).collect();
        move |tool: &str| owned.iter().any(|t| t == tool)
    }

    #[test]
    fn a_single_tool_is_satisfied_when_present() {
        assert!(satisfied("npx", &has(&["npx"])));
    }

    #[test]
    fn a_single_tool_is_unsatisfied_when_absent() {
        assert!(!satisfied("playwright", &has(&["npx"])));
    }

    #[test]
    fn alternatives_are_satisfied_when_any_one_resolves() {
        let present = has(&["chromium"]);
        assert!(satisfied("google-chrome|chromium", &present));
    }

    #[test]
    fn alternatives_are_unsatisfied_when_none_resolves() {
        assert!(!satisfied("google-chrome|chromium", &has(&["firefox"])));
    }

    #[test]
    fn unsatisfied_lists_only_the_requirements_that_failed() {
        let got = unsatisfied(
            &[
                "npx".to_string(),
                "playwright".to_string(),
                "google-chrome|chromium".to_string(),
            ],
            &has(&["npx", "chromium"]),
        );
        assert_eq!(got, vec!["playwright".to_string()]);
    }

    #[test]
    fn resolves_in_finds_an_executable_on_a_fabricated_path() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("faketool");
        fs::write(&bin, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        let path_var = format!("/nonexistent-dir:{}", d.path().display());
        assert!(resolves_in(&path_var, "faketool"));
        assert!(!resolves_in(&path_var, "othertool"));
    }

    #[test]
    fn resolves_in_ignores_a_non_executable_file() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("notexec");
        fs::write(&bin, "data").unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!resolves_in(&d.path().display().to_string(), "notexec"));
    }

    fn skill_dir(body: Option<&str>) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        if let Some(body) = body {
            fs::write(d.path().join("SKILL.md"), body).unwrap();
        }
        d
    }

    #[test]
    fn a_skill_with_name_and_description_is_valid() {
        let d = skill_dir(Some(
            "---\nname: ponytail\ndescription: Be lazy on purpose.\n---\n\nBody.\n",
        ));
        assert_eq!(validate_skill_md(d.path()), Ok(()));
    }

    #[test]
    fn a_missing_skill_md_is_invalid() {
        let d = skill_dir(None);
        let err = validate_skill_md(d.path()).unwrap_err();
        assert!(err.contains("SKILL.md"), "unexpected reason: {err}");
    }

    #[test]
    fn a_skill_md_without_frontmatter_is_invalid() {
        let d = skill_dir(Some("# Just a heading\n\nNo frontmatter here.\n"));
        let err = validate_skill_md(d.path()).unwrap_err();
        assert!(err.contains("frontmatter"), "unexpected reason: {err}");
    }

    #[test]
    fn a_skill_md_without_name_is_invalid() {
        let d = skill_dir(Some("---\ndescription: Only a description.\n---\n"));
        let err = validate_skill_md(d.path()).unwrap_err();
        assert!(err.contains("name"), "unexpected reason: {err}");
    }

    #[test]
    fn a_skill_md_without_description_is_invalid() {
        let d = skill_dir(Some("---\nname: lonely\n---\n"));
        let err = validate_skill_md(d.path()).unwrap_err();
        assert!(err.contains("description"), "unexpected reason: {err}");
    }

    #[test]
    fn an_empty_description_is_invalid() {
        let d = skill_dir(Some("---\nname: lonely\ndescription:   \n---\n"));
        let err = validate_skill_md(d.path()).unwrap_err();
        assert!(err.contains("description"), "unexpected reason: {err}");
    }

    use crate::cli::Scope;

    fn requires(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    fn valid_skill(root: &Path, name: &str) -> Item {
        let dir = root.join("skills").join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: A real one.\n---\n"),
        )
        .unwrap();
        Item::Linked {
            name: name.to_string(),
            kind: Kind::Skill,
            path: dir,
        }
    }

    fn health_of(rows: &[Row], name: &str) -> String {
        format!("{:?}", rows.iter().find(|r| r.name == name).unwrap().health)
    }

    #[test]
    fn a_skill_with_every_tool_present_is_ok() {
        let d = tempfile::tempdir().unwrap();
        let mut items = BTreeMap::new();
        items.insert("good".to_string(), valid_skill(d.path(), "good"));
        let rows = collect(&items, &requires(&[("good", &["npx"])]), &has(&["npx"]));
        assert_eq!(health_of(&rows, "good"), "Ok");
    }

    #[test]
    fn a_skill_with_a_missing_tool_reports_it() {
        let d = tempfile::tempdir().unwrap();
        let mut items = BTreeMap::new();
        items.insert("needy".to_string(), valid_skill(d.path(), "needy"));
        let rows = collect(
            &items,
            &requires(&[("needy", &["npx", "playwright"])]),
            &has(&["npx"]),
        );
        assert_eq!(health_of(&rows, "needy"), r#"Missing(["playwright"])"#);
    }

    #[test]
    fn an_item_with_no_declared_requirements_is_ok() {
        let d = tempfile::tempdir().unwrap();
        let mut items = BTreeMap::new();
        items.insert("plain".to_string(), valid_skill(d.path(), "plain"));
        let rows = collect(&items, &requires(&[]), &has(&[]));
        assert_eq!(health_of(&rows, "plain"), "Ok");
    }

    #[test]
    fn a_broken_skill_md_is_invalid_even_when_its_tools_are_present() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("skills/broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "no frontmatter at all\n").unwrap();
        let mut items = BTreeMap::new();
        items.insert(
            "broken".to_string(),
            Item::Linked {
                name: "broken".to_string(),
                kind: Kind::Skill,
                path: dir,
            },
        );
        let rows = collect(&items, &requires(&[("broken", &["npx"])]), &has(&["npx"]));
        assert!(
            health_of(&rows, "broken").starts_with("Invalid"),
            "got {}",
            health_of(&rows, "broken")
        );
    }

    #[test]
    fn an_installer_is_checked_for_tools_but_not_for_a_skill_md() {
        let mut items = BTreeMap::new();
        items.insert(
            "bmad".to_string(),
            Item::Installer {
                name: "bmad".to_string(),
                command: "npx bmad-method install".to_string(),
                scope: Scope::Project,
            },
        );
        let rows = collect(
            &items,
            &requires(&[("bmad", &["npx", "uv"])]),
            &has(&["npx"]),
        );
        assert_eq!(health_of(&rows, "bmad"), r#"Missing(["uv"])"#);

        let rows = collect(&items, &requires(&[("bmad", &["npx"])]), &has(&["npx"]));
        assert_eq!(health_of(&rows, "bmad"), "Ok");
    }

    #[test]
    fn every_item_gets_exactly_one_row() {
        let d = tempfile::tempdir().unwrap();
        let mut items = BTreeMap::new();
        items.insert("a".to_string(), valid_skill(d.path(), "a"));
        items.insert("b".to_string(), valid_skill(d.path(), "b"));
        let rows = collect(&items, &requires(&[]), &has(&[]));
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn anything_unhealthy_is_detectable_without_formatting() {
        let d = tempfile::tempdir().unwrap();
        let mut items = BTreeMap::new();
        items.insert("needy".to_string(), valid_skill(d.path(), "needy"));
        let rows = collect(&items, &requires(&[("needy", &["gone"])]), &has(&[]));
        assert!(rows.iter().any(|r| !r.health.is_ok()));

        let rows = collect(&items, &requires(&[]), &has(&[]));
        assert!(rows.iter().all(|r| r.health.is_ok()));
    }
}
