use crate::config::Profiles;
use crate::item::Item;
use std::collections::{BTreeMap, BTreeSet};

pub const RESERVED_ALL: &str = "all";

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("`{RESERVED_ALL}` is a reserved profile name; rename the profile in profiles.toml")]
    ReservedName,
    #[error("unknown profile `{0}`")]
    UnknownProfile(String),
    #[error("profile `{profile}` names unknown item `{item}`")]
    UnknownItem { profile: String, item: String },
    #[error("unknown item `{0}`")]
    UnknownSingleItem(String),
    #[error("include cycle: {0}")]
    Cycle(String),
}

pub fn resolve(
    profiles: &Profiles,
    wanted: &[String],
    singles: &[String],
    items: &BTreeMap<String, Item>,
) -> Result<BTreeSet<String>, ProfileError> {
    if profiles.profiles.contains_key(RESERVED_ALL) {
        return Err(ProfileError::ReservedName);
    }

    let mut out = BTreeSet::new();
    for name in wanted {
        if name == RESERVED_ALL {
            out.extend(items.keys().cloned());
            continue;
        }
        let mut stack = Vec::new();
        visit(profiles, name, items, &mut stack, &mut out)?;
    }
    for name in singles {
        if !items.contains_key(name) {
            return Err(ProfileError::UnknownSingleItem(name.clone()));
        }
        out.insert(name.clone());
    }
    Ok(out)
}

fn visit(
    profiles: &Profiles,
    name: &str,
    items: &BTreeMap<String, Item>,
    stack: &mut Vec<String>,
    out: &mut BTreeSet<String>,
) -> Result<(), ProfileError> {
    if stack.iter().any(|s| s == name) {
        stack.push(name.to_string());
        return Err(ProfileError::Cycle(stack.join(" -> ")));
    }
    let profile = profiles
        .profiles
        .get(name)
        .ok_or_else(|| ProfileError::UnknownProfile(name.to_string()))?;

    stack.push(name.to_string());
    for included in &profile.include {
        visit(profiles, included, items, stack, out)?;
    }
    stack.pop();

    for item in &profile.items {
        if !items.contains_key(item) {
            return Err(ProfileError::UnknownItem {
                profile: name.to_string(),
                item: item.clone(),
            });
        }
        out.insert(item.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Profile, Profiles};
    use crate::item::{Item, Kind};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn items(names: &[&str]) -> BTreeMap<String, Item> {
        names
            .iter()
            .map(|n| {
                (
                    n.to_string(),
                    Item::Linked {
                        name: n.to_string(),
                        kind: Kind::Skill,
                        path: PathBuf::from("skills").join(n),
                    },
                )
            })
            .collect()
    }

    fn profiles(entries: &[(&str, &[&str], &[&str])]) -> Profiles {
        let mut p = Profiles::default();
        for (name, include, items) in entries {
            p.profiles.insert(
                name.to_string(),
                Profile {
                    include: include.iter().map(|s| s.to_string()).collect(),
                    items: items.iter().map(|s| s.to_string()).collect(),
                },
            );
        }
        p
    }

    fn set(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn resolves_a_single_profile() {
        let p = profiles(&[("base", &[], &["a"])]);
        let got = resolve(&p, &["base".into()], &[], &items(&["a", "b"])).unwrap();
        assert_eq!(got, set(&["a"]));
    }

    #[test]
    fn include_is_transitive_and_deduplicated() {
        let p = profiles(&[
            ("base", &[], &["a"]),
            ("mid", &["base"], &["b"]),
            ("top", &["mid", "base"], &["c"]),
        ]);
        let got = resolve(&p, &["top".into()], &[], &items(&["a", "b", "c"])).unwrap();
        assert_eq!(got, set(&["a", "b", "c"]));
    }

    #[test]
    fn multiple_profiles_union() {
        let p = profiles(&[("x", &[], &["a"]), ("y", &[], &["b"])]);
        let got = resolve(&p, &["x".into(), "y".into()], &[], &items(&["a", "b"])).unwrap();
        assert_eq!(got, set(&["a", "b"]));
    }

    #[test]
    fn all_expands_to_every_item() {
        let p = profiles(&[("base", &[], &["a"])]);
        let got = resolve(&p, &["all".into()], &[], &items(&["a", "b", "c"])).unwrap();
        assert_eq!(got, set(&["a", "b", "c"]));
    }

    #[test]
    fn rejects_include_cycles() {
        let p = profiles(&[("x", &["y"], &[]), ("y", &["x"], &[])]);
        let err = resolve(&p, &["x".into()], &[], &items(&["a"])).unwrap_err();
        match err {
            ProfileError::Cycle(path) => {
                assert!(path.contains("x"), "cycle path missing 'x': {path}")
            }
            other => panic!("expected Cycle, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_profile() {
        let p = profiles(&[("x", &[], &["a"])]);
        let err = resolve(&p, &["nope".into()], &[], &items(&["a"])).unwrap_err();
        match err {
            ProfileError::UnknownProfile(name) => assert_eq!(name, "nope"),
            other => panic!("expected UnknownProfile, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_item() {
        let p = profiles(&[("x", &[], &["ghost"])]);
        let err = resolve(&p, &["x".into()], &[], &items(&["a"])).unwrap_err();
        match err {
            ProfileError::UnknownItem { profile, item } => {
                assert_eq!(profile, "x");
                assert_eq!(item, "ghost");
            }
            other => panic!("expected UnknownItem, got {other:?}"),
        }
    }

    #[test]
    fn single_items_join_the_profiles_and_must_exist() {
        let p = profiles(&[("x", &[], &["a"])]);
        let got = resolve(&p, &["x".into()], &["b".into()], &items(&["a", "b"])).unwrap();
        assert_eq!(got, set(&["a", "b"]));
        let only = resolve(&p, &[], &["b".into()], &items(&["a", "b"])).unwrap();
        assert_eq!(only, set(&["b"]));
        let err = resolve(&p, &[], &["ghost".into()], &items(&["a"])).unwrap_err();
        assert!(matches!(err, ProfileError::UnknownSingleItem(n) if n == "ghost"));
    }

    // Review Focus 4
    #[test]
    fn rejects_a_user_defined_profile_named_all() {
        let p = profiles(&[("all", &[], &["a"])]);
        let err = resolve(&p, &["all".into()], &[], &items(&["a", "b"])).unwrap_err();
        match err {
            ProfileError::ReservedName => {} // expected
            other => panic!("expected ReservedName, got {other:?}"),
        }
    }
}
