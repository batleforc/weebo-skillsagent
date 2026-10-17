use std::fs;
use std::path::{Path, PathBuf};

fn repo(root: &Path, skills: &[&str], profiles_toml: &str) {
    for s in skills {
        let d = root.join("skills").join(s);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("SKILL.md"), "x").unwrap();
    }
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join("profiles.toml"), profiles_toml).unwrap();
    fs::write(root.join("vendor.toml"), "").unwrap();
}

fn profiles(names: &[&str]) -> wska::selection::Selection {
    wska::selection::Selection {
        profiles: names.iter().map(|s| s.to_string()).collect(),
        items: vec![],
    }
}

fn env_none(_: &str) -> Option<String> {
    None
}

/// The real process environment, as `main.rs` passes it — needed by the tests
/// that exercise PATH lookup for a skill's required tools.
fn env_real(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

#[test]
fn installs_a_profile_then_converges_on_a_second_run() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["ponytail", "graphify", "impeccable"],
        r#"
[profiles.base]
items = ["ponytail", "graphify"]

[profiles.frontend]
include = ["base"]
items = ["impeccable"]
"#,
    );

    let first = wska::app::install(
        &root,
        &profiles(&["frontend"]),
        wska::cli::Scope::Project,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(first.linked, 3);

    let links: PathBuf = root.join(".claude/skills");
    for s in ["ponytail", "graphify", "impeccable"] {
        assert!(fs::symlink_metadata(links.join(s))
            .unwrap()
            .file_type()
            .is_symlink());
    }

    // Idempotent
    let again = wska::app::install(
        &root,
        &profiles(&["frontend"]),
        wska::cli::Scope::Project,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(again.linked, 0);
    assert_eq!(again.unlinked, 0);

    // Declarative: narrowing the profile set removes what is no longer wanted
    let narrowed = wska::app::install(
        &root,
        &profiles(&["base"]),
        wska::cli::Scope::Project,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(narrowed.unlinked, 1);
    assert!(!links.join("impeccable").exists());
    assert!(links.join("ponytail").exists());
}

#[test]
fn refuses_to_replace_a_hand_written_directory_and_changes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["ponytail", "graphify"],
        "[profiles.base]\nitems = [\"ponytail\", \"graphify\"]\n",
    );
    let handmade = root.join(".claude/skills/ponytail");
    fs::create_dir_all(&handmade).unwrap();
    fs::write(handmade.join("SKILL.md"), "mine").unwrap();

    let err = wska::app::install(
        &root,
        &profiles(&["base"]),
        wska::cli::Scope::Project,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap_err();
    assert!(err.to_string().contains("own"), "unexpected error: {err}");

    // Nothing was written, including the item that would have succeeded.
    assert_eq!(
        fs::read_to_string(handmade.join("SKILL.md")).unwrap(),
        "mine"
    );
    assert!(!root.join(".claude/skills/graphify").exists());
}

#[test]
fn install_links_a_skill_whose_required_tool_is_missing_and_warns_about_it() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["needy", "content"],
        "[profiles.base]\nitems = [\"needy\", \"content\"]\n",
    );
    fs::write(
        root.join("vendor.toml"),
        "[requires]\nneedy = [\"definitely-not-installed-anywhere\"]\n",
    )
    .unwrap();

    let got = wska::app::install(
        &root,
        &profiles(&["base"]),
        wska::cli::Scope::Project,
        false,
        false,
        &root,
        &env_real,
    )
    .unwrap();

    // A missing tool must not block the link: the skill is still readable,
    // and the tool may be installed later.
    assert_eq!(got.linked, 2);
    assert!(root.join(".claude/skills/needy").exists());

    // But the user has to be told, naming the item and the tool.
    assert_eq!(got.warnings.len(), 1, "warnings: {:?}", got.warnings);
    let warning = &got.warnings[0];
    assert!(warning.contains("needy"), "warning: {warning}");
    assert!(
        warning.contains("definitely-not-installed-anywhere"),
        "warning: {warning}"
    );
}

#[test]
fn install_warns_about_nothing_when_every_requirement_is_satisfied() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(&root, &["fine"], "[profiles.base]\nitems = [\"fine\"]\n");
    // `sh` exists on any machine that can run this test suite at all.
    fs::write(root.join("vendor.toml"), "[requires]\nfine = [\"sh\"]\n").unwrap();

    let got = wska::app::install(
        &root,
        &profiles(&["base"]),
        wska::cli::Scope::Project,
        false,
        false,
        &root,
        &env_real,
    )
    .unwrap();
    assert_eq!(got.linked, 1);
    assert!(got.warnings.is_empty(), "warnings: {:?}", got.warnings);
}

#[test]
fn doctor_reports_missing_tools_and_unusable_skills() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["needy", "fine"],
        "[profiles.base]\nitems = [\"fine\"]\n",
    );
    // `repo()` writes a stub SKILL.md with no frontmatter, so give the two
    // skills real ones and leave a third deliberately broken.
    for s in ["needy", "fine"] {
        fs::write(
            root.join("skills").join(s).join("SKILL.md"),
            format!("---\nname: {s}\ndescription: Real enough.\n---\n"),
        )
        .unwrap();
    }
    let broken = root.join("skills/broken");
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("SKILL.md"), "no frontmatter\n").unwrap();

    fs::write(
        root.join("vendor.toml"),
        "[requires]\nneedy = [\"definitely-not-installed-anywhere\"]\nfine = [\"sh\"]\n",
    )
    .unwrap();

    let rows = wska::app::doctor(&root, &env_real).unwrap();
    let health = |name: &str| format!("{:?}", rows.iter().find(|r| r.name == name).unwrap().health);

    assert_eq!(health("fine"), "Ok");
    assert_eq!(
        health("needy"),
        r#"Missing(["definitely-not-installed-anywhere"])"#
    );
    assert!(
        health("broken").starts_with("Invalid"),
        "{}",
        health("broken")
    );
    assert_eq!(rows.len(), 3);
}

#[test]
fn records_profiles_and_single_items_and_a_bare_install_reapplies_them() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["ponytail", "techdoc", "other"],
        "[profiles.base]\nitems = [\"ponytail\"]\n",
    );
    let bare = wska::selection::Selection::default();

    // Nothing recorded yet: a bare install refuses rather than unlinking all.
    let err = wska::app::install(
        &root,
        &bare,
        wska::cli::Scope::Project,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap_err();
    assert!(err.to_string().contains("wska.toml"), "{err}");

    let chosen = wska::selection::Selection {
        profiles: vec!["base".into()],
        items: vec!["techdoc".into()],
    };
    let first = wska::app::install(
        &root,
        &chosen,
        wska::cli::Scope::Project,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(first.linked, 2);
    assert!(!root.join(".claude/skills/other").exists());

    let target = root.join(".claude");
    assert_eq!(
        wska::selection::Selection::load(&target).unwrap(),
        Some(chosen)
    );

    // Remove one link: the bare install restores it from wska.toml alone.
    fs::remove_file(root.join(".claude/skills/techdoc")).unwrap();
    let again = wska::app::install(
        &root,
        &bare,
        wska::cli::Scope::Project,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(again.linked, 1);
    assert_eq!(again.unlinked, 0);
}
