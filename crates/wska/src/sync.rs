use crate::config::{External, Provides, Source};
use crate::hash::hash_dir;
use crate::item::Kind;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub enum Outcome {
    Unchanged {
        skill: String,
    },
    FastForward {
        skill: String,
        from: String,
        to: String,
    },
    Patched {
        skill: String,
    },
    MissingLicence {
        source: String,
    },
    /// An external skill was cloned (fresh, or re-cloned by sync).
    Fetched {
        skill: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("git {args} failed: {status}")]
    Git { args: String, status: String },
    #[error("{op} {path}: {source}")]
    Io {
        op: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("vendor.toml has no source named `{0}`")]
    UnknownSource(String),
    #[error("refusing upstream symlink {path}: target `{target}` leaves the skill directory")]
    UnsafeLink { path: PathBuf, target: String },
    #[error("external `{name}`: no SKILL.md at {path}")]
    NotASkill { name: String, path: PathBuf },
    #[error("parsing vendor.toml: {0}")]
    Edit(#[from] toml_edit::TomlError),
}

pub fn sync_source(
    name: &str,
    src: &Source,
    repo_root: &Path,
    cache: &Path,
    check_only: bool,
) -> Result<Vec<Outcome>, SyncError> {
    let mut out = Vec::new();
    if src.licence.is_none() {
        out.push(Outcome::MissingLicence {
            source: name.to_string(),
        });
    }

    let clone = cache.join(name);
    if clone.exists() {
        std::fs::remove_dir_all(&clone).map_err(|source| SyncError::Io {
            op: "clearing",
            path: clone.clone(),
            source,
        })?;
    }
    std::fs::create_dir_all(cache).map_err(|source| SyncError::Io {
        op: "creating",
        path: cache.to_path_buf(),
        source,
    })?;
    git(
        cache,
        &[
            "clone",
            "--quiet",
            "--depth",
            "1",
            "--branch",
            &src.git_ref,
            &src.upstream,
            name,
        ],
    )?;
    let new_commit = git_stdout(&clone, &["rev-parse", "--short", "HEAD"])?;

    let upstream_dir = clone.join(&src.subpath);
    let skills = match &src.provides {
        Provides::Named(v) => v.clone(),
        Provides::Star(_) => {
            let mut v = Vec::new();
            for entry in std::fs::read_dir(&upstream_dir).map_err(|source| SyncError::Io {
                op: "reading",
                path: upstream_dir.clone(),
                source,
            })? {
                let entry = entry.map_err(|source| SyncError::Io {
                    op: "reading",
                    path: upstream_dir.clone(),
                    source,
                })?;
                if entry.path().is_dir() {
                    v.push(entry.file_name().to_string_lossy().to_string());
                }
            }
            v.sort();
            v
        }
    };

    let mut fresh: BTreeMap<String, String> = BTreeMap::new();
    let mut any_patched = false;
    for skill in skills {
        let local = repo_root.join(Kind::Skill.subdir()).join(&skill);
        let remote = upstream_dir.join(&skill);
        let remote_hash = hash_dir(&remote).map_err(|source| SyncError::Io {
            op: "hashing",
            path: remote.clone(),
            source,
        })?;

        if !local.exists() {
            if !check_only {
                copy_dir(&remote, &local)?;
            }
            fresh.insert(skill.clone(), remote_hash.clone());
            out.push(Outcome::FastForward {
                skill,
                from: "absent".into(),
                to: new_commit.clone(),
            });
            continue;
        }

        let local_hash = hash_dir(&local).map_err(|source| SyncError::Io {
            op: "hashing",
            path: local.clone(),
            source,
        })?;
        let recorded = src.hashes.get(&skill);

        if recorded == Some(&local_hash) {
            if local_hash == remote_hash {
                fresh.insert(skill.clone(), local_hash);
                out.push(Outcome::Unchanged { skill });
            } else {
                if !check_only {
                    let tmp = local.with_file_name(format!(".wska-tmp-{skill}"));
                    if tmp.exists() {
                        std::fs::remove_dir_all(&tmp).map_err(|source| SyncError::Io {
                            op: "clearing",
                            path: tmp.clone(),
                            source,
                        })?;
                    }
                    if let Err(e) = copy_dir(&remote, &tmp) {
                        let _ = std::fs::remove_dir_all(&tmp);
                        return Err(e);
                    }
                    // shortcut: remove+rename is not atomic, so a crash between them still loses
                    // the skill; upgrade to a single atomic rename (swap via renameat2) if sync
                    // ever runs somewhere `skills/` is not recoverable from git.
                    std::fs::remove_dir_all(&local).map_err(|source| SyncError::Io {
                        op: "replacing",
                        path: local.clone(),
                        source,
                    })?;
                    std::fs::rename(&tmp, &local).map_err(|source| SyncError::Io {
                        op: "renaming",
                        path: local.clone(),
                        source,
                    })?;
                }
                fresh.insert(skill.clone(), remote_hash);
                out.push(Outcome::FastForward {
                    skill,
                    from: src.commit.clone().unwrap_or_else(|| "unknown".into()),
                    to: new_commit.clone(),
                });
            }
        } else {
            // Patched locally: keep the recorded hash so the next sync still
            // sees the patch, and never overwrite.
            if let Some(r) = recorded {
                fresh.insert(skill.clone(), r.clone());
            }
            out.push(Outcome::Patched { skill });
            any_patched = true;
        }
    }

    if !check_only {
        // A patched skill means the tree is not actually at new_commit for
        // this source, so don't advance commit — leave it as recorded.
        let commit = if any_patched {
            None
        } else {
            Some(new_commit.as_str())
        };
        write_back(&repo_root.join("vendor.toml"), name, commit, &fresh)?;
    }
    Ok(out)
}

/// Update one source's `hashes` in place, preserving comments and field
/// order. `commit` is set to the given value when `Some`; when `None` the
/// existing `commit` field (if any) is left untouched.
pub fn write_back(
    vendor_path: &Path,
    source: &str,
    commit: Option<&str>,
    hashes: &BTreeMap<String, String>,
) -> Result<(), SyncError> {
    let body = std::fs::read_to_string(vendor_path).map_err(|e| SyncError::Io {
        op: "reading",
        path: vendor_path.to_path_buf(),
        source: e,
    })?;
    let mut doc: toml_edit::DocumentMut = body.parse()?;
    let table = doc
        .get_mut("sources")
        .and_then(|s| s.get_mut(source))
        .ok_or_else(|| SyncError::UnknownSource(source.to_string()))?;

    if let Some(commit) = commit {
        table["commit"] = toml_edit::value(commit);
    }
    let mut inline = toml_edit::InlineTable::new();
    for (k, v) in hashes {
        inline.insert(k, v.clone().into());
    }
    table["hashes"] = toml_edit::Item::Value(toml_edit::Value::InlineTable(inline));

    std::fs::write(vendor_path, doc.to_string()).map_err(|e| SyncError::Io {
        op: "writing",
        path: vendor_path.to_path_buf(),
        source: e,
    })
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), SyncError> {
    copy_into(from, to, to)
}

/// Whether a symlink placed at `link` pointing at `target` stays under `root`.
/// Resolved lexically, not via canonicalize: the link is copied before
/// whatever it points at, so the real path does not exist yet.
fn link_stays_inside(link: &Path, target: &Path, root: &Path) -> bool {
    use std::path::Component;
    if target.is_absolute() {
        return false;
    }
    let Some(base) = link.parent() else {
        return false;
    };
    let Ok(rel) = base.strip_prefix(root) else {
        return false;
    };
    let mut depth = 0usize;
    for component in rel.components().chain(target.components()) {
        match component {
            Component::CurDir => {}
            Component::Normal(_) => depth += 1,
            Component::ParentDir => match depth.checked_sub(1) {
                Some(d) => depth = d,
                None => return false,
            },
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

fn copy_into(from: &Path, to: &Path, root: &Path) -> Result<(), SyncError> {
    std::fs::create_dir_all(to).map_err(|source| SyncError::Io {
        op: "creating",
        path: to.to_path_buf(),
        source,
    })?;
    for entry in std::fs::read_dir(from).map_err(|source| SyncError::Io {
        op: "reading",
        path: from.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| SyncError::Io {
            op: "reading",
            path: from.to_path_buf(),
            source,
        })?;
        let dest = to.join(entry.file_name());
        let md = std::fs::symlink_metadata(entry.path()).map_err(|source| SyncError::Io {
            op: "inspecting",
            path: entry.path(),
            source,
        })?;
        if md.file_type().is_symlink() {
            let target = std::fs::read_link(entry.path()).map_err(|source| SyncError::Io {
                op: "reading link",
                path: entry.path(),
                source,
            })?;
            // Upstream is untrusted input: a link out of the tree would be
            // handed to the agent verbatim once the skill is installed.
            if !link_stays_inside(&dest, &target, root) {
                return Err(SyncError::UnsafeLink {
                    path: entry.path(),
                    target: target.display().to_string(),
                });
            }
            std::os::unix::fs::symlink(target, &dest).map_err(|source| SyncError::Io {
                op: "linking",
                path: dest,
                source,
            })?;
        } else if md.is_dir() {
            copy_into(&entry.path(), &dest, root)?;
        } else {
            std::fs::copy(entry.path(), &dest).map_err(|source| SyncError::Io {
                op: "copying to",
                path: dest,
                source,
            })?;
        }
    }
    Ok(())
}

/// Clone an external skill's repo into `cache/<name>` unless it is already
/// there; `refresh` re-clones. Returns whether a clone happened.
// shortcut: no commit/hash tracking, so `--check` cannot see drift; add it when an external needs pinning.
pub fn fetch_external(
    name: &str,
    ext: &External,
    cache: &Path,
    refresh: bool,
) -> Result<bool, SyncError> {
    let clone = cache.join(name);
    let fetched = refresh || !clone.exists();
    if fetched {
        if clone.exists() {
            std::fs::remove_dir_all(&clone).map_err(|source| SyncError::Io {
                op: "clearing",
                path: clone.clone(),
                source,
            })?;
        }
        std::fs::create_dir_all(cache).map_err(|source| SyncError::Io {
            op: "creating",
            path: cache.to_path_buf(),
            source,
        })?;
        git(
            cache,
            &[
                "clone",
                "--quiet",
                "--depth",
                "1",
                "--branch",
                &ext.git_ref,
                &ext.upstream,
                name,
            ],
        )?;
    }
    let skill_md = clone.join(&ext.path).join("SKILL.md");
    if !skill_md.is_file() {
        return Err(SyncError::NotASkill {
            name: name.to_string(),
            path: skill_md,
        });
    }
    Ok(fetched)
}

fn git(cwd: &Path, args: &[&str]) -> Result<(), SyncError> {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|source| SyncError::Io {
            op: "spawning git in",
            path: cwd.to_path_buf(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(SyncError::Git {
            args: args.join(" "),
            status: status.to_string(),
        })
    }
}

fn git_stdout(cwd: &Path, args: &[&str]) -> Result<String, SyncError> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|source| SyncError::Io {
            op: "spawning git in",
            path: cwd.to_path_buf(),
            source,
        })?;
    if !out.status.success() {
        return Err(SyncError::Git {
            args: args.join(" "),
            status: out.status.to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// What a `--check` run concluded.
#[derive(Debug)]
pub enum CheckVerdict {
    Clean,
    /// Unpatched skills whose upstream has moved on.
    Drift(Vec<String>),
    /// Sources with no licence recorded.
    LicenceMissing(Vec<String>),
}

impl CheckVerdict {
    /// 1 for an unmet obligation, 2 for mere staleness, so a CI job can tell
    /// "fix this" apart from "there is an update to review".
    pub fn exit_code(&self) -> i32 {
        match self {
            CheckVerdict::Clean => 0,
            CheckVerdict::LicenceMissing(_) => 1,
            CheckVerdict::Drift(_) => 2,
        }
    }
}

/// Classify a `--check` run. A missing licence outranks drift: one is an
/// obligation unmet, the other only an available update. A patched skill is
/// never a failure — the divergence is the author's own deliberate edit, and
/// failing on it would leave `task audit` permanently red for anyone who
/// patches anything.
pub fn check_verdict(outcomes: &[Outcome]) -> CheckVerdict {
    let unlicensed: Vec<String> = outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            Outcome::MissingLicence { source } => Some(source.clone()),
            _ => None,
        })
        .collect();
    if !unlicensed.is_empty() {
        return CheckVerdict::LicenceMissing(unlicensed);
    }

    let drifted: Vec<String> = outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            Outcome::FastForward { skill, .. } => Some(skill.clone()),
            _ => None,
        })
        .collect();
    if drifted.is_empty() {
        CheckVerdict::Clean
    } else {
        CheckVerdict::Drift(drifted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Provides, Source};
    use std::collections::BTreeMap;
    use std::fs;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    /// An upstream repository with skills/<name>/SKILL.md on branch main.
    fn upstream(dir: &Path, body: &str) {
        fs::create_dir_all(dir.join("skills/brainstorming")).unwrap();
        fs::write(dir.join("skills/brainstorming/SKILL.md"), body).unwrap();
        fs::write(dir.join("LICENSE"), "MIT").unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-qm", "x"]);
    }

    fn source(upstream_dir: &Path) -> Source {
        Source {
            upstream: format!("file://{}", upstream_dir.display()),
            git_ref: "main".into(),
            subpath: "skills".into(),
            provides: Provides::Star("*".into()),
            licence: Some("MIT".into()),
            commit: None,
            hashes: BTreeMap::new(),
        }
    }

    fn external(upstream_dir: &Path, path: &str) -> External {
        External {
            upstream: format!("file://{}", upstream_dir.display()),
            git_ref: "main".into(),
            path: path.into(),
            licence: Some("MIT".into()),
        }
    }

    #[test]
    fn fetch_external_clones_once_then_reuses() {
        let d = tempfile::tempdir().unwrap();
        let up = d.path().join("up");
        upstream(&up, "---\nname: b\n---\n");
        let cache = d.path().join("cache");
        let ext = external(&up, "skills/brainstorming");
        assert!(fetch_external("b", &ext, &cache, false).unwrap());
        assert!(cache.join("b/skills/brainstorming/SKILL.md").is_file());
        assert!(!fetch_external("b", &ext, &cache, false).unwrap());
        assert!(fetch_external("b", &ext, &cache, true).unwrap());
    }

    #[test]
    fn fetch_external_rejects_a_path_without_skill_md() {
        let d = tempfile::tempdir().unwrap();
        let up = d.path().join("up");
        upstream(&up, "x");
        let err = fetch_external(
            "b",
            &external(&up, "skills"),
            &d.path().join("cache"),
            false,
        )
        .unwrap_err();
        assert!(matches!(err, SyncError::NotASkill { .. }), "got {err:?}");
    }

    #[test]
    fn fast_forwards_an_unpatched_skill() {
        let d = tempfile::tempdir().unwrap();
        let up = d.path().join("up");
        upstream(&up, "v1");
        let repo = d.path().join("repo");
        let local = repo.join("skills/brainstorming");
        fs::create_dir_all(&local).unwrap();
        fs::write(local.join("SKILL.md"), "v1").unwrap();

        let mut src = source(&up);
        src.hashes.insert(
            "brainstorming".into(),
            crate::hash::hash_dir(&local).unwrap(),
        );
        // upstream moves on
        fs::write(up.join("skills/brainstorming/SKILL.md"), "v2").unwrap();
        git(&up, &["commit", "-aqm", "v2"]);

        // R4: sync_source(check_only = false) ends by calling write_back,
        // which reads repo_root/vendor.toml. Create it so that path runs.
        fs::write(
            repo.join("vendor.toml"),
            r#"[sources.sp]
upstream = "unused"
ref = "main"
subpath = "skills"
provides = "*"
licence = "MIT"
"#,
        )
        .unwrap();

        let out = sync_source("sp", &src, &repo, &d.path().join("cache"), false).unwrap();
        assert!(
            out.iter().any(|o| matches!(o, Outcome::FastForward { .. })),
            "{out:?}"
        );
        assert_eq!(fs::read_to_string(local.join("SKILL.md")).unwrap(), "v2");
    }

    #[test]
    fn refuses_to_overwrite_a_patched_skill() {
        let d = tempfile::tempdir().unwrap();
        let up = d.path().join("up");
        upstream(&up, "v1");
        let repo = d.path().join("repo");
        let local = repo.join("skills/brainstorming");
        fs::create_dir_all(&local).unwrap();
        fs::write(local.join("SKILL.md"), "v1").unwrap();

        let mut src = source(&up);
        src.hashes.insert(
            "brainstorming".into(),
            crate::hash::hash_dir(&local).unwrap(),
        );
        // the author patches locally
        fs::write(local.join("SKILL.md"), "v1 + my edit").unwrap();
        // and upstream moves on
        fs::write(up.join("skills/brainstorming/SKILL.md"), "v2").unwrap();
        git(&up, &["commit", "-aqm", "v2"]);

        // R4: same reason as above — write_back reads repo_root/vendor.toml.
        fs::write(
            repo.join("vendor.toml"),
            r#"[sources.sp]
upstream = "unused"
ref = "main"
subpath = "skills"
provides = "*"
licence = "MIT"
"#,
        )
        .unwrap();

        let out = sync_source("sp", &src, &repo, &d.path().join("cache"), false).unwrap();
        assert!(
            out.iter().any(|o| matches!(o, Outcome::Patched { .. })),
            "{out:?}"
        );
        assert_eq!(
            fs::read_to_string(local.join("SKILL.md")).unwrap(),
            "v1 + my edit",
            "a patched skill must not be overwritten"
        );
    }

    /// R18: a patched skill means the tree isn't really at upstream's new
    /// commit, so `commit` in vendor.toml must not advance to it either.
    #[test]
    fn patched_skill_does_not_advance_the_recorded_commit() {
        let d = tempfile::tempdir().unwrap();
        let up = d.path().join("up");
        upstream(&up, "v1");
        let repo = d.path().join("repo");
        let local = repo.join("skills/brainstorming");
        fs::create_dir_all(&local).unwrap();
        fs::write(local.join("SKILL.md"), "v1").unwrap();

        let mut src = source(&up);
        let old_hash = crate::hash::hash_dir(&local).unwrap();
        src.hashes.insert("brainstorming".into(), old_hash.clone());
        src.commit = Some("oldsha".into());
        // the author patches locally
        fs::write(local.join("SKILL.md"), "v1 + my edit").unwrap();
        // and upstream moves on
        fs::write(up.join("skills/brainstorming/SKILL.md"), "v2").unwrap();
        git(&up, &["commit", "-aqm", "v2"]);
        let new_commit = String::from_utf8(
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .current_dir(&up)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();

        fs::write(
            repo.join("vendor.toml"),
            r#"[sources.sp]
upstream = "unused"
ref = "main"
subpath = "skills"
provides = "*"
licence = "MIT"
commit = "oldsha"
"#,
        )
        .unwrap();

        let out = sync_source("sp", &src, &repo, &d.path().join("cache"), false).unwrap();
        assert!(
            out.iter().any(|o| matches!(o, Outcome::Patched { .. })),
            "{out:?}"
        );

        let body = fs::read_to_string(repo.join("vendor.toml")).unwrap();
        assert!(body.contains("commit = \"oldsha\""), "{body}");
        assert!(!body.contains(&new_commit), "{body}");
        assert!(body.contains(&old_hash), "{body}");
    }

    #[test]
    fn check_only_reports_a_missing_licence_and_writes_nothing() {
        let d = tempfile::tempdir().unwrap();
        let up = d.path().join("up");
        upstream(&up, "v1");
        let repo = d.path().join("repo");
        let local = repo.join("skills/brainstorming");
        fs::create_dir_all(&local).unwrap();
        fs::write(local.join("SKILL.md"), "v1").unwrap();

        let mut src = source(&up);
        src.licence = None;
        src.hashes.insert(
            "brainstorming".into(),
            crate::hash::hash_dir(&local).unwrap(),
        );
        fs::write(up.join("skills/brainstorming/SKILL.md"), "v2").unwrap();
        git(&up, &["commit", "-aqm", "v2"]);

        let out = sync_source("sp", &src, &repo, &d.path().join("cache"), true).unwrap();
        assert!(
            out.iter()
                .any(|o| matches!(o, Outcome::MissingLicence { .. })),
            "{out:?}"
        );
        assert_eq!(fs::read_to_string(local.join("SKILL.md")).unwrap(), "v1");
    }

    #[test]
    fn an_escaping_upstream_symlink_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let from = d.path().join("remote");
        std::fs::create_dir_all(from.join("nested")).unwrap();
        std::fs::write(from.join("SKILL.md"), "x").unwrap();
        std::os::unix::fs::symlink("/etc/shadow", from.join("secrets")).unwrap();
        let err = copy_dir(&from, &d.path().join("local")).unwrap_err();
        assert!(matches!(err, SyncError::UnsafeLink { .. }), "got {err:?}");

        std::fs::remove_file(from.join("secrets")).unwrap();
        std::os::unix::fs::symlink("../../../etc/shadow", from.join("nested/up")).unwrap();
        let err = copy_dir(&from, &d.path().join("local2")).unwrap_err();
        assert!(matches!(err, SyncError::UnsafeLink { .. }), "got {err:?}");
    }

    #[test]
    fn an_internal_upstream_symlink_is_copied() {
        let d = tempfile::tempdir().unwrap();
        let from = d.path().join("remote");
        std::fs::create_dir_all(from.join("nested")).unwrap();
        std::fs::write(from.join("SKILL.md"), "x").unwrap();
        std::os::unix::fs::symlink("../SKILL.md", from.join("nested/alias")).unwrap();
        let to = d.path().join("local");
        copy_dir(&from, &to).unwrap();
        assert_eq!(
            std::fs::read_link(to.join("nested/alias")).unwrap(),
            Path::new("../SKILL.md")
        );
    }

    #[test]
    fn write_back_preserves_comments() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("vendor.toml");
        fs::write(
            &p,
            "# keep me\n[sources.sp]\nupstream = \"x\"\nref = \"main\"\nsubpath = \"skills\"\nprovides = \"*\"\nlicence = \"MIT\"\n",
        )
        .unwrap();
        let mut hashes = BTreeMap::new();
        hashes.insert("brainstorming".to_string(), "sha256:ab".to_string());
        write_back(&p, "sp", Some("deadbee"), &hashes).unwrap();
        let body = fs::read_to_string(&p).unwrap();
        assert!(body.contains("# keep me"), "{body}");
        assert!(body.contains("deadbee"), "{body}");
        assert!(body.contains("sha256:ab"), "{body}");
    }

    #[test]
    fn an_all_unchanged_run_is_clean() {
        let outcomes = vec![
            Outcome::Unchanged { skill: "a".into() },
            Outcome::Unchanged { skill: "b".into() },
        ];
        assert!(matches!(check_verdict(&outcomes), CheckVerdict::Clean));
    }

    #[test]
    fn a_fast_forward_is_drift_naming_the_skill() {
        let outcomes = vec![
            Outcome::Unchanged { skill: "a".into() },
            Outcome::FastForward {
                skill: "b".into(),
                from: "old".into(),
                to: "new".into(),
            },
        ];
        match check_verdict(&outcomes) {
            CheckVerdict::Drift(skills) => assert_eq!(skills, vec!["b".to_string()]),
            other => panic!("expected Drift, got {other:?}"),
        }
    }

    #[test]
    fn a_patched_skill_is_clean_because_divergence_is_deliberate() {
        let outcomes = vec![Outcome::Patched {
            skill: "mine".into(),
        }];
        assert!(
            matches!(check_verdict(&outcomes), CheckVerdict::Clean),
            "a skill the author patched must not fail the check"
        );
    }

    #[test]
    fn a_missing_licence_outranks_drift() {
        let outcomes = vec![
            Outcome::FastForward {
                skill: "b".into(),
                from: "old".into(),
                to: "new".into(),
            },
            Outcome::MissingLicence {
                source: "unlicensed".into(),
            },
        ];
        match check_verdict(&outcomes) {
            CheckVerdict::LicenceMissing(sources) => {
                assert_eq!(sources, vec!["unlicensed".to_string()])
            }
            other => panic!("expected LicenceMissing, got {other:?}"),
        }
    }

    #[test]
    fn verdicts_carry_their_own_exit_code() {
        assert_eq!(CheckVerdict::Clean.exit_code(), 0);
        assert_eq!(CheckVerdict::Drift(vec!["b".into()]).exit_code(), 2);
        assert_eq!(
            CheckVerdict::LicenceMissing(vec!["x".into()]).exit_code(),
            1
        );
    }
}
