use crate::apply::{apply, Applied, ApplyError};
use crate::cli::Scope;
use crate::{config, item, plan, profile, target};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    Scan(#[from] item::ScanError),
    #[error(transparent)]
    Profile(#[from] profile::ProfileError),
    #[error(transparent)]
    Target(#[from] target::TargetError),
    #[error(transparent)]
    Apply(#[from] ApplyError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Refused(String),
    #[error(transparent)]
    Sync(#[from] crate::sync::SyncError),
}

pub fn install(
    repo_root: &Path,
    profiles: &[String],
    scope: Scope,
    dry_run: bool,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Applied, AppError> {
    // Canonicalize once, up front: item paths and the ownership comparison
    // base must share a prefix or nothing ever classifies as already-correct.
    let repo_root = target::repo_root(repo_root)?;

    let profiles_cfg = config::load_profiles(&repo_root.join("profiles.toml"))?;
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    let items = item::scan(&repo_root, &vendor)?;
    let wanted = profile::resolve(&profiles_cfg, profiles, &items)?;

    let target_dir = target::resolve(scope, cwd, env)?;

    let built = plan::build(&items, &wanted, &target_dir, &repo_root, scope)?;
    if let Err(e) = built.validate() {
        for refusal in &built.refusals {
            eprintln!("refuse {}: {}", refusal.path.display(), refusal.reason);
        }
        return Err(AppError::Refused(e.to_string()));
    }
    Ok(apply(&built, dry_run)?)
}

pub fn status(
    repo_root: &Path,
    scope: Scope,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Vec<crate::status::Row>, AppError> {
    // Canonicalize once, up front, same as install: item::scan and
    // status::collect must share the same comparison base or nothing
    // classifies as Linked.
    let root = target::repo_root(repo_root)?;
    let vendor = config::load_vendor(&root.join("vendor.toml"))?;
    let items = item::scan(&root, &vendor)?;
    let target_dir = target::resolve(scope, cwd, env)?;
    Ok(crate::status::collect(&items, &target_dir, &root)?)
}

pub fn list(repo_root: &Path, profiles: bool) -> Result<Vec<String>, AppError> {
    if profiles {
        let cfg = config::load_profiles(&repo_root.join("profiles.toml"))?;
        return Ok(cfg.profiles.keys().cloned().collect());
    }
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    Ok(item::scan(repo_root, &vendor)?.keys().cloned().collect())
}

pub fn sync(
    repo_root: &Path,
    sources: &[String],
    check: bool,
) -> Result<Vec<crate::sync::Outcome>, AppError> {
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    let cache = repo_root.join(".task/wska-cache");
    let mut out = Vec::new();
    for (name, source) in &vendor.sources {
        if !sources.is_empty() && !sources.contains(name) {
            continue;
        }
        out.extend(crate::sync::sync_source(
            name, source, repo_root, &cache, check,
        )?);
    }
    Ok(out)
}
