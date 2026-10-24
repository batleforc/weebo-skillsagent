#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]

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
        repos: vec![],
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
        wska::cli::Agent::Claude,
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
        wska::cli::Agent::Claude,
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
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(narrowed.unlinked, 1);
    assert!(!links.join("impeccable").exists());
    assert!(links.join("ponytail").exists());

    // The project's .gitignore lists exactly the links left
    let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(ignore.contains("/.claude/skills/ponytail\n"));
    assert!(!ignore.contains("impeccable"));
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
        wska::cli::Agent::Claude,
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
        wska::cli::Agent::Claude,
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
        wska::cli::Agent::Claude,
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

    let rows = wska::app::doctor(
        &root,
        wska::cli::Scope::Project,
        wska::cli::Agent::Claude,
        &root,
        &env_real,
    )
    .unwrap();
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
        wska::cli::Agent::Claude,
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
        repos: vec![],
    };
    let first = wska::app::install(
        &root,
        &chosen,
        wska::cli::Scope::Project,
        wska::cli::Agent::Claude,
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
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(again.linked, 1);
    assert_eq!(again.unlinked, 0);
}

#[test]
fn add_extends_the_recorded_selection_instead_of_replacing_it() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["ponytail", "techdoc", "other"],
        "[profiles.base]\nitems = [\"ponytail\"]\n",
    );
    let scope = wska::cli::Scope::Project;
    wska::app::install(
        &root,
        &profiles(&["base"]),
        scope,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();

    let extra = wska::selection::Selection {
        profiles: vec!["base".into()], // already recorded: not duplicated
        items: vec!["techdoc".into()],
        repos: vec![],
    };
    let added = wska::app::add(
        &root,
        &extra,
        scope,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(added.linked, 1);
    assert_eq!(added.unlinked, 0);
    assert!(fs::symlink_metadata(root.join(".claude/skills/ponytail")).is_ok());
    assert_eq!(
        wska::selection::Selection::load(&root.join(".claude")).unwrap(),
        Some(wska::selection::Selection {
            profiles: vec!["base".into()],
            items: vec!["techdoc".into()],
            repos: vec![],
        })
    );

    let bare = wska::selection::Selection::default();
    assert!(wska::app::add(
        &root,
        &bare,
        scope,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none
    )
    .is_err());
}

#[test]
fn remove_drops_recorded_names_and_refuses_unrecorded_ones() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["ponytail", "techdoc"],
        "[profiles.base]\nitems = [\"ponytail\"]\n",
    );
    let scope = wska::cli::Scope::Project;
    let sel = |profiles: &[&str], items: &[&str]| wska::selection::Selection {
        profiles: profiles.iter().map(|s| s.to_string()).collect(),
        items: items.iter().map(|s| s.to_string()).collect(),
        repos: vec![],
    };
    let target = root.join(".claude");

    // Nothing recorded yet.
    assert!(wska::app::remove(
        &root,
        &sel(&[], &["techdoc"]),
        scope,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none
    )
    .is_err());

    wska::app::install(
        &root,
        &sel(&["base"], &["techdoc"]),
        scope,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();

    // ponytail comes from `base`, not from --item: refused, nothing changes.
    let err = wska::app::remove(
        &root,
        &sel(&[], &["ponytail"]),
        scope,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap_err();
    assert!(err.to_string().contains("item `ponytail`"), "{err}");
    assert!(fs::symlink_metadata(root.join(".claude/skills/ponytail")).is_ok());

    let removed = wska::app::remove(
        &root,
        &sel(&[], &["techdoc"]),
        scope,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(removed.unlinked, 1);
    assert_eq!(
        wska::selection::Selection::load(&target).unwrap(),
        Some(sel(&["base"], &[]))
    );

    // Removing the last name unlinks everything and records an empty selection.
    let emptied = wska::app::remove(
        &root,
        &sel(&["base"], &[]),
        scope,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(emptied.unlinked, 1);
    assert_eq!(
        wska::selection::Selection::load(&target).unwrap(),
        Some(sel(&[], &[]))
    );
}

#[test]
fn list_tables_show_profiles_per_item_and_the_recorded_selection() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["ponytail", "techdoc", "loose"],
        "[profiles.base]\nitems = [\"ponytail\"]\n\n[profiles.docs]\ninclude = [\"base\"]\nitems = [\"techdoc\", \"ponytail\"]\n",
    );
    let scope = wska::cli::Scope::Project;
    let items = wska::app::list(
        &root,
        false,
        scope,
        wska::cli::Agent::Claude,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(items.headers, ["ITEM", "KIND", "INSTALLED", "PROFILES"]);
    let row =
        |t: &wska::app::Table, name: &str| t.rows.iter().find(|r| r[0] == name).unwrap().clone();
    assert_eq!(row(&items, "ponytail")[3], "base, docs");
    assert_eq!(row(&items, "loose")[3], "-");
    assert_eq!(row(&items, "techdoc")[1], "skill");
    assert_eq!(row(&items, "techdoc")[2], "-");
    assert!(
        items
            .to_string()
            .starts_with("ITEM      KIND   INSTALLED  PROFILES\n"),
        "{items}"
    );

    assert!(
        wska::app::list_recorded(&root, scope, wska::cli::Agent::Claude, &root, &env_none).is_err()
    );
    let sel = wska::selection::Selection {
        profiles: vec!["base".into()],
        items: vec!["loose".into()],
        repos: vec![],
    };
    wska::app::install(
        &root,
        &sel,
        scope,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();
    let after = wska::app::list(
        &root,
        false,
        scope,
        wska::cli::Agent::Claude,
        &root,
        &env_none,
    )
    .unwrap();
    assert_eq!(row(&after, "ponytail")[2], "yes");
    assert_eq!(row(&after, "loose")[2], "yes");
    assert_eq!(row(&after, "techdoc")[2], "-");

    let recorded =
        wska::app::list_recorded(&root, scope, wska::cli::Agent::Claude, &root, &env_none).unwrap();
    assert_eq!(
        recorded.rows,
        vec![
            vec!["base".to_string(), "profile".into(), "ponytail".into()],
            vec!["loose".to_string(), "item".into(), "loose".into()],
        ]
    );
}

/// `repo()` plus a real commit, so the directory can be `git clone`d by path.
fn git_repo(root: &Path, skills: &[&str], profiles_toml: &str) -> String {
    repo(root, skills, profiles_toml);
    fs::remove_dir_all(root.join(".git")).unwrap();
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
            .current_dir(root)
            .status()
            .unwrap()
            .success();
        assert!(ok);
    }
    root.to_str().unwrap().to_string()
}

#[test]
fn an_extra_repo_merges_its_items_and_profiles_and_is_kept_until_removed() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["ponytail"],
        "[profiles.base]\nitems = [\"ponytail\"]\n",
    );
    // Its profile names an item of the primary repo: one namespace.
    let url = git_repo(
        &d.path().join("team"),
        &["team-a"],
        "[profiles.team]\nitems = [\"team-a\", \"ponytail\"]\n",
    );
    let data = d.path().join("data").to_str().unwrap().to_string();
    let env = move |k: &str| (k == "XDG_DATA_HOME").then(|| data.clone());
    let sel = |profiles: &[&str], repos: &[&str]| wska::selection::Selection {
        profiles: profiles.iter().map(|s| s.to_string()).collect(),
        items: vec![],
        repos: repos.iter().map(|s| s.to_string()).collect(),
    };
    let install = |s: &wska::selection::Selection| {
        wska::app::install(
            &root,
            s,
            wska::cli::Scope::Project,
            wska::cli::Agent::Claude,
            false,
            false,
            &root,
            &env,
        )
        .unwrap()
    };
    let links = root.join(".claude/skills");

    assert_eq!(install(&sel(&["team"], &[&url])).linked, 2);
    assert!(links.join("team-a/SKILL.md").exists());
    assert!(fs::read_link(links.join("team-a"))
        .unwrap()
        .starts_with(d.path().join("data/wska/repos").canonicalize().unwrap()));

    // Naming profiles anew keeps the recorded repo.
    let narrowed = install(&sel(&["base"], &[]));
    assert_eq!(narrowed.unlinked, 1);
    let recorded = wska::selection::Selection::load(&root.join(".claude"))
        .unwrap()
        .unwrap();
    assert_eq!(recorded.repos, vec![url.clone()]);

    // Dropping the repo unlinks what came from it: those links stay owned.
    install(&sel(&["team"], &[]));
    let removed = wska::app::remove(
        &root,
        &sel(&["team"], &[&url]),
        wska::cli::Scope::Project,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env,
    )
    .unwrap();
    assert_eq!(removed.unlinked, 2);
    assert!(fs::symlink_metadata(links.join("team-a")).is_err());
}

#[test]
fn an_item_defined_by_two_repos_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(&root, &["ponytail"], "");
    let url = git_repo(&d.path().join("team"), &["ponytail"], "");
    let data = d.path().join("data").to_str().unwrap().to_string();
    let env = move |k: &str| (k == "XDG_DATA_HOME").then(|| data.clone());
    let err = wska::app::install(
        &root,
        &wska::selection::Selection {
            profiles: vec![],
            items: vec!["ponytail".into()],
            repos: vec![url],
        },
        wska::cli::Scope::Project,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env,
    )
    .unwrap_err();
    assert!(err.to_string().contains("item `ponytail`"), "{err}");
}

#[test]
fn ignores_links_in_the_repo_gitignore_when_dot_claude_is_a_symlink() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("repo");
    repo(
        &root,
        &["ponytail"],
        "[profiles.base]\nitems = [\"ponytail\"]\n",
    );
    let elsewhere = d.path().join("dotfiles/claude");
    fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, root.join(".claude")).unwrap();

    wska::app::install(
        &root,
        &profiles(&["base"]),
        wska::cli::Scope::Project,
        wska::cli::Agent::Claude,
        false,
        false,
        &root,
        &env_none,
    )
    .unwrap();

    let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(ignore.contains("/.claude/skills/ponytail\n"));
    assert!(!d.path().join("dotfiles/.gitignore").exists());
}
