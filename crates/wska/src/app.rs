use crate::apply::{apply, Applied, ApplyError};
use crate::cli::Scope;
use crate::selection::{Selection, SelectionError};
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
    #[error(transparent)]
    Selection(#[from] SelectionError),
}

/// Link `selection`, or the one recorded in the target when it is empty, and
/// record what was applied so a bare `install`/`update` can re-apply it.
pub fn install(
    repo_root: &Path,
    selection: &Selection,
    scope: Scope,
    dry_run: bool,
    run_installers: bool,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Applied, AppError> {
    let selection = if selection.is_empty() {
        let target_dir = target::resolve(scope, cwd, env)?;
        Selection::load(&target_dir)?
            .filter(|s| !s.is_empty())
            .ok_or_else(|| SelectionError::Missing(target_dir.join(crate::selection::FILE)))?
    } else {
        selection.clone()
    };
    apply_selection(
        repo_root,
        &selection,
        scope,
        dry_run,
        run_installers,
        cwd,
        env,
    )
}

/// Link exactly `selection` — empty means unlink everything wska owns there —
/// and record it in the target's wska.toml.
fn apply_selection(
    repo_root: &Path,
    selection: &Selection,
    scope: Scope,
    dry_run: bool,
    run_installers: bool,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Applied, AppError> {
    // Canonicalize once, up front: item paths and the ownership comparison
    // base must share a prefix or nothing ever classifies as already-correct.
    let repo_root = target::repo_root(repo_root)?;

    let profiles_cfg = config::load_profiles(&repo_root.join("profiles.toml"))?;
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    let items = item::scan(&repo_root, &vendor)?;
    let target_dir = target::resolve(scope, cwd, env)?;
    let wanted = profile::resolve(&profiles_cfg, &selection.profiles, &selection.items, &items)?;

    // Externals must be on disk before planning, or their links classify as
    // pointing at nothing. Clone-if-missing keeps a second install a no-op.
    if !dry_run {
        let cache = repo_root.join(".task/wska-cache/external");
        for (name, ext) in &vendor.external {
            if wanted.contains(name) && crate::sync::fetch_external(name, ext, &cache, false)? {
                println!("fetch  {name}: {}", ext.upstream);
            }
        }
    }

    let built = plan::build(
        &items,
        &wanted,
        &target_dir,
        &repo_root,
        scope,
        run_installers,
    )?;
    if let Err(e) = built.validate() {
        for refusal in &built.refusals {
            eprintln!("refuse {}: {}", refusal.path.display(), refusal.reason);
        }
        return Err(AppError::Refused(e.to_string()));
    }
    // Advisory, never blocking: a skill whose tool is absent is still
    // readable, and the tool may be installed later. Computed here rather
    // than inside plan::build so the planner needs no PATH resolver.
    let path_var = env("PATH").unwrap_or_default();
    let has = |tool: &str| crate::doctor::resolves_in(&path_var, tool);
    let mut warnings = Vec::new();
    for name in &wanted {
        let missing = crate::doctor::unsatisfied(
            vendor.requires.get(name).map(Vec::as_slice).unwrap_or(&[]),
            &has,
        );
        if !missing.is_empty() {
            warnings.push(format!("{name}: missing {}", missing.join(", ")));
        }
    }
    for warning in &warnings {
        eprintln!("warn   {warning}");
    }

    let mut applied = apply(&built, dry_run)?;
    applied.warnings = warnings;
    if !dry_run {
        selection.save(&target_dir)?;
    }
    Ok(applied)
}

/// Install the target's recorded selection plus `extra`, so adding one item
/// does not mean re-typing everything already there.
pub fn add(
    repo_root: &Path,
    extra: &Selection,
    scope: Scope,
    dry_run: bool,
    run_installers: bool,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Applied, AppError> {
    if extra.is_empty() {
        return Err(AppError::Refused(
            "nothing to add: pass --profile or --item".into(),
        ));
    }
    let recorded = Selection::load(&target::resolve(scope, cwd, env)?)?.unwrap_or_default();
    install(
        repo_root,
        &recorded.merged(extra),
        scope,
        dry_run,
        run_installers,
        cwd,
        env,
    )
}

/// Install the target's recorded selection minus `gone`. Every name must be
/// recorded: an item that only comes through a recorded profile would stay
/// linked, so that is refused rather than silently doing nothing.
pub fn remove(
    repo_root: &Path,
    gone: &Selection,
    scope: Scope,
    dry_run: bool,
    run_installers: bool,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Applied, AppError> {
    if gone.is_empty() {
        return Err(AppError::Refused(
            "nothing to remove: pass --profile or --item".into(),
        ));
    }
    let target_dir = target::resolve(scope, cwd, env)?;
    let recorded = Selection::load(&target_dir)?
        .ok_or_else(|| SelectionError::Missing(target_dir.join(crate::selection::FILE)))?;
    let absent: Vec<String> = gone
        .profiles
        .iter()
        .filter(|n| !recorded.profiles.contains(n))
        .map(|n| format!("profile `{n}`"))
        .chain(
            gone.items
                .iter()
                .filter(|n| !recorded.items.contains(n))
                .map(|n| format!("item `{n}`")),
        )
        .collect();
    if !absent.is_empty() {
        return Err(AppError::Refused(format!(
            "not recorded in {}: {} (an item that comes from a profile can only go with that profile)",
            target_dir.join(crate::selection::FILE).display(),
            absent.join(", ")
        )));
    }
    let kept = Selection {
        profiles: recorded
            .profiles
            .iter()
            .filter(|n| !gone.profiles.contains(n))
            .cloned()
            .collect(),
        items: recorded
            .items
            .iter()
            .filter(|n| !gone.items.contains(n))
            .cloned()
            .collect(),
    };
    apply_selection(repo_root, &kept, scope, dry_run, run_installers, cwd, env)
}

/// Pull the items repo, then re-link `selection`, or the target's recorded one
/// if `selection` is empty. Links are symlinks into the repo, so the pull
/// alone refreshes linked items; the re-install picks up items newly added to
/// a profile, and installers. With nothing given and nothing recorded, it
/// only pulls.
pub fn update(
    repo_root: &Path,
    selection: &Selection,
    scope: Scope,
    run_installers: bool,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<Applied>, AppError> {
    target::pull(repo_root)?;
    if selection.is_empty() && Selection::load(&target::resolve(scope, cwd, env)?)?.is_none() {
        return Ok(None);
    }
    install(repo_root, selection, scope, false, run_installers, cwd, env).map(Some)
}

/// Whether each item in the repo can actually work here: its declared tools
/// resolve on PATH, and (for skills) its SKILL.md is usable. Deliberately
/// scope-free — whether a link is in place is what `status` answers.
pub fn doctor(
    repo_root: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Vec<crate::doctor::Row>, AppError> {
    let repo_root = target::repo_root(repo_root)?;
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    let items = item::scan(&repo_root, &vendor)?;

    let path_var = env("PATH").unwrap_or_default();
    let has = |tool: &str| crate::doctor::resolves_in(&path_var, tool);
    Ok(crate::doctor::collect(&items, &vendor.requires, &has))
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

/// Rows under a header, printed with every column but the last padded.
#[derive(Debug, PartialEq, Eq)]
pub struct Table {
    pub headers: [&'static str; 3],
    pub rows: Vec<[String; 3]>,
}

impl std::fmt::Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let width = |col: usize| {
            self.rows
                .iter()
                .map(|r| r[col].len())
                .chain([self.headers[col].len()])
                .max()
                .unwrap_or(0)
        };
        let (w0, w1) = (width(0), width(1));
        let headers = self.headers.map(String::from);
        for [a, b, c] in std::iter::once(&headers).chain(&self.rows) {
            writeln!(f, "{a:<w0$}  {b:<w1$}  {c}")?;
        }
        Ok(())
    }
}

fn names_or_dash(names: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let joined = names
        .into_iter()
        .map(|n| n.as_ref().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if joined.is_empty() {
        "-".into()
    } else {
        joined
    }
}

/// Every item with its kind and the profiles that list it directly, or every
/// profile with what it includes and lists.
pub fn list(repo_root: &Path, profiles: bool) -> Result<Table, AppError> {
    let cfg = config::load_profiles(&repo_root.join("profiles.toml"))?;
    if profiles {
        let rows = cfg
            .profiles
            .iter()
            .map(|(name, p)| {
                [
                    name.clone(),
                    names_or_dash(&p.include),
                    names_or_dash(&p.items),
                ]
            })
            .collect();
        return Ok(Table {
            headers: ["PROFILE", "INCLUDES", "ITEMS"],
            rows,
        });
    }
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    let rows = item::scan(repo_root, &vendor)?
        .values()
        .map(|it| {
            let kind = match it {
                item::Item::Linked { kind, .. } => match kind {
                    item::Kind::Skill => "skill",
                    item::Kind::Agent => "agent",
                    item::Kind::Mcp => "mcp",
                },
                item::Item::Installer { .. } => "installer",
            };
            let in_profiles = cfg
                .profiles
                .iter()
                .filter(|(_, p)| p.items.iter().any(|i| i == it.name()))
                .map(|(name, _)| name);
            [
                it.name().to_string(),
                kind.to_string(),
                names_or_dash(in_profiles),
            ]
        })
        .collect();
    Ok(Table {
        headers: ["ITEM", "KIND", "PROFILES"],
        rows,
    })
}

/// The target's recorded selection, each profile expanded to the items it
/// brings (includes followed).
pub fn list_recorded(
    repo_root: &Path,
    scope: Scope,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Table, AppError> {
    let target_dir = target::resolve(scope, cwd, env)?;
    let path = target_dir.join(crate::selection::FILE);
    let recorded = Selection::load(&target_dir)?
        .ok_or_else(|| AppError::Refused(format!("no selection recorded in {}", path.display())))?;
    let cfg = config::load_profiles(&repo_root.join("profiles.toml"))?;
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    let items = item::scan(repo_root, &vendor)?;
    let mut rows = Vec::new();
    for name in &recorded.profiles {
        let brings = profile::resolve(&cfg, std::slice::from_ref(name), &[], &items)?;
        rows.push([name.clone(), "profile".into(), names_or_dash(&brings)]);
    }
    for name in &recorded.items {
        rows.push([name.clone(), "item".into(), name.clone()]);
    }
    Ok(Table {
        headers: ["NAME", "RECORDED AS", "ITEMS"],
        rows,
    })
}

pub fn sync(
    repo_root: &Path,
    sources: &[String],
    check: bool,
) -> Result<Vec<crate::sync::Outcome>, AppError> {
    let vendor = config::load_vendor(&repo_root.join("vendor.toml"))?;
    // A name that matches nothing would otherwise sync nothing and report
    // Clean, so a typo reads as "everything is up to date".
    for name in sources {
        if !vendor.sources.contains_key(name) && !vendor.external.contains_key(name) {
            return Err(crate::sync::SyncError::UnknownSource(name.clone()).into());
        }
    }
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
    // Externals carry no hashes, so --check has nothing to compare and only
    // the licence is reported; a real sync re-clones them.
    for (name, ext) in &vendor.external {
        if !sources.is_empty() && !sources.contains(name) {
            continue;
        }
        if ext.licence.is_none() {
            out.push(crate::sync::Outcome::MissingLicence {
                source: name.clone(),
            });
        }
        if !check {
            crate::sync::fetch_external(name, ext, &cache.join("external"), true)?;
            out.push(crate::sync::Outcome::Fetched {
                skill: name.clone(),
            });
        }
    }
    Ok(out)
}
