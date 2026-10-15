use sha2::{Digest, Sha256};
use std::io;
use std::path::{Path, PathBuf};

/// SHA-256 over directory entries sorted by relative path. Each entry
/// contributes: relative path bytes, 0x00, a kind byte (b'f' file, b'l'
/// symlink), content length as u64 LE, contents. For a symlink the contents
/// are the link's target text; links are never followed. Modes, timestamps and
/// ownership are excluded.
pub fn hash_dir(root: &Path) -> io::Result<String> {
    let mut entries: Vec<(Vec<u8>, u8, Vec<u8>)> = Vec::new();
    collect(root, Path::new(""), &mut entries)?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut h = Sha256::new();
    for (rel, kind, body) in entries {
        h.update(&rel);
        h.update([0u8]);
        h.update([kind]);
        h.update((body.len() as u64).to_le_bytes());
        h.update(&body);
    }
    Ok(format!("sha256:{}", hex::encode(h.finalize())))
}

fn collect(
    dir: &Path,
    rel_dir: &Path,
    out: &mut Vec<(Vec<u8>, u8, Vec<u8>)>,
) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let rel: PathBuf = rel_dir.join(&name);
        // symlink_metadata never follows, so a link to a directory is recorded
        // as a link and not descended into. That is what terminates cycles.
        let md = std::fs::symlink_metadata(entry.path())?;
        if md.file_type().is_symlink() {
            let target = std::fs::read_link(entry.path())?;
            out.push((
                rel.as_os_str().as_encoded_bytes().to_vec(),
                b'l',
                target.as_os_str().as_encoded_bytes().to_vec(),
            ));
        } else if md.is_dir() {
            collect(&entry.path(), &rel, out)?;
        } else {
            out.push((
                rel.as_os_str().as_encoded_bytes().to_vec(),
                b'f',
                std::fs::read(entry.path())?,
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn write(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    #[test]
    fn stable_across_reruns() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "SKILL.md", "hello");
        write(d.path(), "references/a.md", "a");
        assert_eq!(hash_dir(d.path()).unwrap(), hash_dir(d.path()).unwrap());
    }

    #[test]
    fn content_change_changes_hash() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "SKILL.md", "hello");
        let before = hash_dir(d.path()).unwrap();
        write(d.path(), "SKILL.md", "hello!");
        assert_ne!(before, hash_dir(d.path()).unwrap());
    }

    #[test]
    fn mtime_change_does_not_change_hash() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "SKILL.md", "hello");
        let before = hash_dir(d.path()).unwrap();
        let p = d.path().join("SKILL.md");
        let body = fs::read(&p).unwrap();
        fs::remove_file(&p).unwrap();
        fs::write(&p, body).unwrap();
        assert_eq!(before, hash_dir(d.path()).unwrap());
    }

    #[test]
    fn two_dirs_with_same_content_hash_equal() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        for d in [a.path(), b.path()] {
            write(d, "z.md", "z");
            write(d, "a.md", "a");
        }
        assert_eq!(hash_dir(a.path()).unwrap(), hash_dir(b.path()).unwrap());
    }

    // Review Focus 5
    #[test]
    fn symlink_is_hashed_as_target_text_and_never_followed() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "SKILL.md", "hello");
        symlink("./definitely-not-here", d.path().join("escape")).unwrap();
        // Must not error despite target not existing (proves link is not followed).
        let h = hash_dir(d.path()).unwrap();
        assert!(h.starts_with("sha256:"));

        // A file whose contents equal the link text must hash differently.
        let e = tempfile::tempdir().unwrap();
        write(e.path(), "SKILL.md", "hello");
        write(e.path(), "escape", "./definitely-not-here");
        assert_ne!(h, hash_dir(e.path()).unwrap());
    }

    #[test]
    fn symlink_cycle_terminates() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "SKILL.md", "hello");
        fs::create_dir(d.path().join("sub")).unwrap();
        symlink(d.path(), d.path().join("sub/loop")).unwrap();
        assert!(hash_dir(d.path()).is_ok());
    }
}
