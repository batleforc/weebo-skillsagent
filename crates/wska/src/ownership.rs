use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub enum Entry {
    /// Nothing exists at this path.
    Absent,
    /// A link we own, already pointing at the expected item.
    OwnedCorrect,
    /// A link we own, pointing at a different item in this repo.
    OwnedWrongTarget { points_to: PathBuf },
    /// A link whose stored target is under this repo but no longer exists.
    OwnedDangling { points_to: PathBuf },
    /// A link pointing outside this repo. Never touched.
    Foreign { points_to: PathBuf },
    /// A real file or directory, not a link. Never touched.
    RealPath,
}

pub fn classify(entry: &Path, repo_root: &Path, expected: Option<&Path>) -> io::Result<Entry> {
    let md = match std::fs::symlink_metadata(entry) {
        Ok(md) => md,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Entry::Absent),
        Err(e) => return Err(e),
    };

    if !md.file_type().is_symlink() {
        return Ok(Entry::RealPath);
    }

    let stored = std::fs::read_link(entry)?;
    // A relative target is relative to the link's own directory, not to the
    // process working directory.
    let resolved = if stored.is_absolute() {
        normalize(&stored)
    } else {
        normalize(&entry.parent().unwrap_or(Path::new("/")).join(&stored))
    };
    let repo_root = normalize(repo_root);

    if !resolved.starts_with(&repo_root) {
        return Ok(Entry::Foreign {
            points_to: resolved,
        });
    }
    if !resolved.exists() {
        return Ok(Entry::OwnedDangling {
            points_to: resolved,
        });
    }
    match expected {
        Some(want) if normalize(want) == resolved => Ok(Entry::OwnedCorrect),
        _ => Ok(Entry::OwnedWrongTarget {
            points_to: resolved,
        }),
    }
}

/// Lexical normalization: resolves `.` and `..` without touching the
/// filesystem, so a relative link into a repo is comparable to an absolute
/// repo path.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    struct Fixture {
        _dir: tempfile::TempDir,
        repo: PathBuf,
        target: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let target = dir.path().join("proj/.claude/skills");
        fs::create_dir_all(repo.join("skills/ponytail")).unwrap();
        fs::create_dir_all(repo.join("skills/other")).unwrap();
        fs::create_dir_all(&target).unwrap();
        Fixture {
            _dir: dir,
            repo: repo.canonicalize().unwrap(),
            target: target.canonicalize().unwrap(),
        }
    }

    #[test]
    fn absent_when_nothing_is_there() {
        let f = fixture();
        let got = classify(
            &f.target.join("ponytail"),
            &f.repo,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(matches!(got, Entry::Absent));
    }

    #[test]
    fn owned_correct_for_an_absolute_link_to_the_expected_item() {
        let f = fixture();
        symlink(f.repo.join("skills/ponytail"), f.target.join("ponytail")).unwrap();
        let got = classify(
            &f.target.join("ponytail"),
            &f.repo,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(matches!(got, Entry::OwnedCorrect));
    }

    // Review Focus 1
    #[test]
    fn owned_correct_for_a_relative_link_resolved_against_the_link_parent() {
        let f = fixture();
        let rel = pathdiff_relative(&f.target, &f.repo.join("skills/ponytail"));
        symlink(&rel, f.target.join("ponytail")).unwrap();
        let got = classify(
            &f.target.join("ponytail"),
            &f.repo,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(
            matches!(got, Entry::OwnedCorrect),
            "relative link misclassified as {got:?}"
        );
    }

    #[test]
    fn owned_wrong_target_when_it_points_at_another_item_here() {
        let f = fixture();
        symlink(f.repo.join("skills/other"), f.target.join("ponytail")).unwrap();
        let got = classify(
            &f.target.join("ponytail"),
            &f.repo,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(matches!(got, Entry::OwnedWrongTarget { .. }));
    }

    #[test]
    fn owned_dangling_when_the_target_under_the_repo_is_gone() {
        let f = fixture();
        symlink(f.repo.join("skills/gone"), f.target.join("gone")).unwrap();
        let got = classify(&f.target.join("gone"), &f.repo, None).unwrap();
        assert!(matches!(got, Entry::OwnedDangling { .. }));
    }

    #[test]
    fn foreign_for_a_link_outside_the_repo() {
        let f = fixture();
        let outside = f.repo.parent().unwrap().join("elsewhere");
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, f.target.join("ponytail")).unwrap();
        let got = classify(
            &f.target.join("ponytail"),
            &f.repo,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(matches!(got, Entry::Foreign { .. }));
    }

    #[test]
    fn foreign_for_an_absolute_link_that_escapes_the_repo_via_dotdot() {
        let f = fixture();
        let outside = f.repo.parent().unwrap().join("outside");
        fs::create_dir_all(&outside).unwrap();
        symlink(f.repo.join("../outside"), f.target.join("ponytail")).unwrap();
        let got = classify(
            &f.target.join("ponytail"),
            &f.repo,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(
            matches!(got, Entry::Foreign { .. }),
            "dotdot escape misclassified as {got:?}"
        );
    }

    #[test]
    fn owned_correct_when_repo_root_itself_contains_a_dotdot() {
        let f = fixture();
        symlink(f.repo.join("skills/ponytail"), f.target.join("ponytail")).unwrap();
        let messy_root = f.repo.join("skills/other/../..");
        let got = classify(
            &f.target.join("ponytail"),
            &messy_root,
            Some(&f.repo.join("skills/ponytail")),
        )
        .unwrap();
        assert!(matches!(got, Entry::OwnedCorrect), "got {got:?}");
    }

    #[test]
    fn real_path_for_a_hand_written_directory() {
        let f = fixture();
        fs::create_dir(f.target.join("handmade")).unwrap();
        let got = classify(&f.target.join("handmade"), &f.repo, None).unwrap();
        assert!(matches!(got, Entry::RealPath));
    }

    /// Minimal relative-path helper for the test above; the implementation
    /// does not need one because it only ever writes absolute links.
    fn pathdiff_relative(from: &Path, to: &Path) -> PathBuf {
        let mut up = PathBuf::new();
        let mut base = from.to_path_buf();
        loop {
            if let Ok(rest) = to.strip_prefix(&base) {
                return up.join(rest);
            }
            if !base.pop() {
                return to.to_path_buf();
            }
            up.push("..");
        }
    }
}
