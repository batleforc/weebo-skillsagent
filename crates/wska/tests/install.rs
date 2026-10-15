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

fn env_none(_: &str) -> Option<String> {
    None
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
        &["frontend".to_string()],
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
        &["frontend".to_string()],
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
        &["base".to_string()],
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
        &["base".to_string()],
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
