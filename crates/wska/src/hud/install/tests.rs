use super::*;
use serde_json::json;
use std::fs;

const OURS: &str = "wska hud statusline";

fn bak(p: &Path) -> std::path::PathBuf {
    p.with_file_name("settings.json.weebo-hud-bak")
}

#[test]
fn missing_file_is_created() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sub/settings.json");
    let msg = install(&p).unwrap();
    assert_eq!(msg, "statusLine: installed");
    let text = fs::read_to_string(&p).unwrap();
    assert!(text.ends_with("}\n"));
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        v["statusLine"],
        json!({"type":"command","command":OURS,"refreshInterval":5})
    );
    assert!(!bak(&p).exists());
}

#[test]
fn other_keys_and_order_are_kept() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("settings.json");
    fs::write(&p, "{\"zeta\":1,\"alpha\":{\"b\":2}}").unwrap();
    install(&p).unwrap();
    let v: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
    let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["zeta", "alpha", "statusLine"]);
    assert_eq!(
        fs::read_to_string(bak(&p)).unwrap(),
        "{\"zeta\":1,\"alpha\":{\"b\":2}}"
    );
}

#[test]
fn foreign_status_line_is_replaced_and_backed_up() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("settings.json");
    let old =
        "{\"statusLine\":{\"type\":\"command\",\"command\":\"bash ~/.claude/statusline.sh\"}}";
    fs::write(&p, old).unwrap();
    let msg = install(&p).unwrap();
    assert_eq!(
        msg,
        format!(
            "statusLine: replaced \"bash ~/.claude/statusline.sh\"\nbackup: {}",
            bak(&p).display()
        )
    );
    assert_eq!(fs::read_to_string(bak(&p)).unwrap(), old);
}

#[test]
fn already_installed_is_a_no_op() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("settings.json");
    install(&p).unwrap();
    let before = fs::read_to_string(&p).unwrap();
    assert_eq!(install(&p).unwrap(), "statusLine: already installed");
    assert_eq!(fs::read_to_string(&p).unwrap(), before);
    assert!(!bak(&p).exists());
}

#[test]
fn existing_backup_is_not_overwritten() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("settings.json");
    fs::write(&p, "{\"a\":1}").unwrap();
    fs::write(bak(&p), "precious").unwrap();
    let msg = install(&p).unwrap();
    assert_eq!(fs::read_to_string(bak(&p)).unwrap(), "precious");
    assert!(!msg.contains("backup:"));
}

#[test]
fn invalid_json_is_an_error_and_left_untouched() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("settings.json");
    fs::write(&p, "{ not json").unwrap();
    assert!(install(&p).is_err());
    assert!(uninstall(&p).is_err());
    assert_eq!(fs::read_to_string(&p).unwrap(), "{ not json");
    assert!(!bak(&p).exists());
}

#[test]
fn uninstall_removes_ours_only() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("settings.json");
    fs::write(&p, "{\"a\":1}").unwrap();
    install(&p).unwrap();
    assert_eq!(uninstall(&p).unwrap(), "statusLine: removed");
    let v: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
    assert_eq!(v, json!({"a":1}));

    let foreign = "{\"statusLine\":{\"type\":\"command\",\"command\":\"other\"}}";
    fs::write(&p, foreign).unwrap();
    assert!(uninstall(&p).unwrap().contains("not ours"));
    assert_eq!(fs::read_to_string(&p).unwrap(), foreign);

    let missing = d.path().join("none.json");
    assert!(uninstall(&missing).unwrap().contains("not ours"));
    assert!(!missing.exists());
}

#[test]
fn write_atomic_replaces_and_leaves_no_temp() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("f");
    write_atomic(&p, b"one").unwrap();
    write_atomic(&p, b"two").unwrap();
    assert_eq!(fs::read(&p).unwrap(), b"two");
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 1);
}

#[test]
fn symlinked_settings_are_followed_and_the_link_kept() {
    let d = tempfile::tempdir().unwrap();
    fs::create_dir(d.path().join("real")).unwrap();
    let real = d.path().join("real/s.json");
    let link = d.path().join("link.json");
    fs::write(&real, "{\"a\":1}").unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    install(&link).unwrap();
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    let v: Value = serde_json::from_str(&fs::read_to_string(&real).unwrap()).unwrap();
    assert_eq!(v["statusLine"]["command"], OURS);
    assert!(d.path().join("link.json.weebo-hud-bak").exists());
    uninstall(&link).unwrap();
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn file_mode_is_kept_on_the_file_and_the_backup() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("settings.json");
    fs::write(&p, "{\"a\":1}").unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
    install(&p).unwrap();
    let mode = |f: &Path| fs::metadata(f).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&p), 0o600);
    assert_eq!(mode(&bak(&p)), 0o600);
}

#[test]
fn no_config_dir_and_no_home_is_an_error() {
    let o = |s: &str| Some(OsString::from(s));
    assert!(settings_path(None, None).is_err());
    assert!(settings_path(o(""), o("")).is_err());
    assert_eq!(
        settings_path(o("/c"), None).unwrap(),
        Path::new("/c/settings.json")
    );
    assert_eq!(
        settings_path(None, o("/h")).unwrap(),
        Path::new("/h/.claude/settings.json")
    );
}
