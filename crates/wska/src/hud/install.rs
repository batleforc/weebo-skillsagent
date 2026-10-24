//! `wska hud install` / `uninstall`: edit the `statusLine` key of Claude Code's settings.json
//! in place (key order kept), so the installer needs no jq.

use super::write_atomic;
use crate::app::AppError;
use serde_json::{json, Map, Value};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const COMMAND: &str = "wska hud statusline";

pub fn default_settings_path() -> Result<PathBuf, AppError> {
    settings_path(
        std::env::var_os("CLAUDE_CONFIG_DIR"),
        std::env::var_os("HOME"),
    )
}

/// Empty counts as unset: never fall back to a path relative to the cwd.
fn settings_path(
    config_dir: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, AppError> {
    let set = |v: Option<OsString>| v.filter(|d| !d.is_empty());
    if let Some(d) = set(config_dir) {
        return Ok(PathBuf::from(d).join("settings.json"));
    }
    set(home)
        .map(|h| PathBuf::from(h).join(".claude/settings.json"))
        .ok_or_else(|| {
            AppError::Refused("neither CLAUDE_CONFIG_DIR nor HOME is set; pass --settings".into())
        })
}

fn wanted() -> Value {
    json!({"type": "command", "command": COMMAND, "refreshInterval": 5})
}

/// The parsed settings (empty object when the file is missing) and the raw text, if any.
fn load(path: &Path) -> Result<(Map<String, Value>, Option<String>), AppError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((Map::new(), None)),
        Err(e) => return Err(e.into()),
    };
    match serde_json::from_str(&text) {
        Ok(Value::Object(m)) => Ok((m, Some(text))),
        Ok(_) => Err(AppError::Refused(format!(
            "{}: not a JSON object",
            path.display()
        ))),
        Err(e) => Err(AppError::Refused(format!(
            "{}: invalid JSON: {e}",
            path.display()
        ))),
    }
}

fn save(path: &Path, map: Map<String, Value>) -> Result<(), AppError> {
    let mut text = serde_json::to_string_pretty(&Value::Object(map))
        .map_err(|e| AppError::Refused(e.to_string()))?;
    text.push('\n');
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    Ok(write_atomic(path, text.as_bytes())?)
}

pub fn install(named: &Path) -> Result<String, AppError> {
    // Follow a symlink so the real file is updated and the link kept.
    let path = &named.canonicalize().unwrap_or_else(|_| named.to_path_buf());
    let (mut map, raw) = load(path)?;
    let old = map.get("statusLine").cloned();
    if old.as_ref() == Some(&wanted()) {
        return Ok("statusLine: already installed".into());
    }
    let mut msg = match &old {
        None => "statusLine: installed".to_string(),
        Some(v) => format!("statusLine: replaced {}", v.get("command").unwrap_or(v)),
    };
    if raw.is_some() {
        let name = named
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        let bak = named.with_file_name(format!("{name}.weebo-hud-bak"));
        if !bak.exists() {
            std::fs::copy(path, &bak)?; // copies the mode too
            msg.push_str(&format!("\nbackup: {}", bak.display()));
        }
    }
    map.insert("statusLine".into(), wanted());
    save(path, map)?;
    Ok(msg)
}

/// CLI entry: `settings` or the default path, then install or uninstall.
pub fn run(install_it: bool, settings: Option<PathBuf>) -> Result<String, AppError> {
    let path = match settings {
        Some(p) => p,
        None => default_settings_path()?,
    };
    if install_it {
        install(&path)
    } else {
        uninstall(&path)
    }
}

pub fn uninstall(named: &Path) -> Result<String, AppError> {
    let path = &named.canonicalize().unwrap_or_else(|_| named.to_path_buf());
    let (mut map, _) = load(path)?;
    let ours = map
        .get("statusLine")
        .and_then(|v| v.get("command"))
        .and_then(Value::as_str)
        == Some(COMMAND);
    if !ours {
        return Ok("statusLine: not ours, left alone".into());
    }
    map.shift_remove("statusLine");
    save(path, map)?;
    Ok("statusLine: removed".into())
}

#[cfg(test)]
mod tests;
