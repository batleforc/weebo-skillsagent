//! `wska hud statusline`: reads Claude Code's statusLine JSON on stdin, prints one powerline line.
//! It never fails: unreadable input or metrics degrade to `—` and the exit code stays 0.

pub mod install;
pub mod metrics;
pub mod render;

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Created with the target's mode from the start, so a 0600 file is never readable by others, even briefly.
#[cfg(unix)]
fn create_tmp(tmp: &Path, perms: Option<&std::fs::Permissions>) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(perms.map_or(0o666, PermissionsExt::mode))
        .open(tmp)
}

/// shortcut: no creation-time mode off unix; the permissions are copied after the write.
#[cfg(not(unix))]
fn create_tmp(tmp: &Path, _perms: Option<&std::fs::Permissions>) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tmp)
}

/// Temp file in the same directory, then rename: readers never see a half-written file.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.tmp{}", std::process::id()));
    let perms = std::fs::metadata(path).ok().map(|m| m.permissions());
    std::fs::remove_file(&tmp).ok(); // a stale temp from a crashed run
    let res = create_tmp(&tmp, perms.as_ref())
        .and_then(|mut f| f.write_all(bytes))
        // The umask may have narrowed the creation mode: restore it exactly.
        .and_then(|()| perms.map_or(Ok(()), |p| std::fs::set_permissions(&tmp, p)))
        .and_then(|()| std::fs::rename(&tmp, path));
    if res.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    res
}

fn state_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir).join("weebo-hud-cpu");
    }
    state_fallback()
}

#[cfg(unix)]
fn state_fallback() -> PathBuf {
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata("/proc/self")
        .map(|m| m.uid())
        .unwrap_or(0);
    PathBuf::from(format!("/tmp/weebo-hud-cpu-{uid}"))
}

#[cfg(not(unix))]
fn state_fallback() -> PathBuf {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default();
    std::env::temp_dir().join(format!("weebo-hud-cpu-{user}"))
}

pub fn statusline() {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw).ok();
    let input = serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
    let at = |p: &str| {
        input
            .pointer(p)
            .and_then(serde_json::Value::as_str)
            .map(String::from)
    };
    let cwd = at("/workspace/current_dir").or_else(|| at("/cwd"));
    let o = render::Options::from_env();
    let home = std::env::var("HOME").ok().filter(|_| o.disque_home);
    let paths: Vec<String> = cwd.into_iter().chain(home).collect();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let m = metrics::sample(&paths, &state_path(), now_ms);
    let line = render::render(&render::build_segments(&input, &m, &o));
    std::io::stdout().write_all(line.as_bytes()).ok();
}
