use crate::item::Kind;
use crate::ownership::{classify, Entry};
use std::io;
use std::path::{Path, PathBuf};

/// Rewrite the block of `<root>/.gitignore` that lists wska's links in
/// `target`, the agent's directory there (named `dir`, `.claude`, ...): they
/// are absolute paths into a clone and mean nothing on another machine.
/// Lists each link rather than the whole directory so hand-written skills and
/// agents stay tracked. Returns whether the file changed.
pub fn sync(root: &Path, dir: &str, target: &Path, owned: &[PathBuf]) -> io::Result<bool> {
    let mut links = Vec::new();
    for kind in Kind::all() {
        let read = match std::fs::read_dir(target.join(kind.subdir())) {
            Ok(r) => r,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        for entry in read {
            let entry = entry?;
            if matches!(
                classify(&entry.path(), owned, None)?,
                Entry::OwnedCorrect | Entry::OwnedWrongTarget { .. } | Entry::OwnedDangling { .. }
            ) {
                links.push(format!(
                    "/{dir}/{}/{}",
                    kind.subdir(),
                    entry.file_name().to_string_lossy()
                ));
            }
        }
    }
    links.sort();

    let path = root.join(".gitignore");
    let old = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let new = with_block(&old, dir, &links).ok_or_else(|| {
        io::Error::other(format!(
            "{}: wska's block for {dir}/ has lost its end marker, fix it by hand",
            path.display()
        ))
    })?;
    if new == old {
        return Ok(false);
    }
    std::fs::write(&path, new)?;
    Ok(true)
}

/// `text` with wska's block for `dir` replaced by `links`, in place, or
/// appended when absent; no links removes the block. `None` when the block
/// has lost its end marker: guessing where it ends could eat the user's lines.
fn with_block(text: &str, dir: &str, links: &[String]) -> Option<String> {
    let begin = format!("# >>> wska links in {dir}/ (rewritten by wska install)");
    let end = format!("# <<< wska links in {dir}/");
    let lines: Vec<&str> = text.lines().collect();
    let mut block = Vec::new();
    if !links.is_empty() {
        block.push(begin.as_str());
        block.extend(links.iter().map(String::as_str));
        block.push(end.as_str());
    }
    let out: Vec<&str> = match lines.iter().position(|l| *l == begin) {
        Some(s) => {
            let e = lines.iter().skip(s).position(|l| *l == end)?;
            let after = lines.iter().skip(s.saturating_add(e).saturating_add(1));
            // Removing the block also drops the blank line appending put before it.
            let before = match s.checked_sub(1) {
                Some(p) if block.is_empty() && lines.get(p) == Some(&"") => p,
                _ => s,
            };
            lines
                .iter()
                .take(before)
                .chain(&block)
                .chain(after)
                .copied()
                .collect()
        }
        None if block.is_empty() => return Some(text.to_string()),
        None if lines.last().is_none_or(|l| l.is_empty()) => {
            lines.iter().chain(&block).copied().collect()
        }
        None => lines.iter().chain(&[""]).chain(&block).copied().collect(),
    };
    let mut joined = out.join("\n");
    if !joined.is_empty() {
        joined.push('\n');
    }
    Some(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links(names: &[&str]) -> Vec<String> {
        names
            .iter()
            .map(|n| format!("/.claude/skills/{n}"))
            .collect()
    }

    #[test]
    fn appends_then_replaces_in_place_then_removes() {
        let added = with_block("target/\n", ".claude", &links(&["a", "b"])).unwrap();
        assert_eq!(
            added,
            "target/\n\n# >>> wska links in .claude/ (rewritten by wska install)\n\
             /.claude/skills/a\n/.claude/skills/b\n# <<< wska links in .claude/\n"
        );
        let edited = format!("{added}dist/\n");
        let replaced = with_block(&edited, ".claude", &links(&["c"])).unwrap();
        assert_eq!(
            replaced,
            "target/\n\n# >>> wska links in .claude/ (rewritten by wska install)\n\
             /.claude/skills/c\n# <<< wska links in .claude/\ndist/\n"
        );
        assert_eq!(
            with_block(&replaced, ".claude", &[]).unwrap(),
            "target/\ndist/\n"
        );
    }

    #[test]
    fn leaves_other_agents_blocks_and_absent_files_alone() {
        let claude = with_block("", ".claude", &links(&["a"])).unwrap();
        let both = with_block(&claude, ".gemini", &["/.gemini/skills/a".into()]).unwrap();
        assert!(both.starts_with(&claude));
        assert_eq!(with_block(&both, ".gemini", &[]).unwrap(), claude);
        assert_eq!(with_block("", ".claude", &[]).unwrap(), "");
    }

    #[test]
    fn refuses_a_block_without_its_end_marker() {
        let added = with_block("", ".claude", &links(&["a"])).unwrap();
        let broken = added.replace("# <<< wska links in .claude/\n", "node_modules/\n");
        assert_eq!(with_block(&broken, ".claude", &links(&["b"])), None);
    }
}
