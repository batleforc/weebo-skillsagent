"""Re-read what a finding cites, from disk. The agent's own quotes are never
trusted, and only files the repo tracks are read: an agent pointing at
`.env` or outside the root must not get its content sent to Jev."""

import subprocess
from pathlib import Path


def excerpt(root: Path, file: str, line: int | None, radius: int = 10) -> str | None:
    root = root.resolve()
    path = (root / file).resolve()
    if not path.is_relative_to(root) or not path.is_file():
        return None
    rel = str(path.relative_to(root))
    tracked = subprocess.run(
        ["git", "ls-files", "--error-unmatch", "--", rel],
        check=False,
        cwd=root,
        capture_output=True,
    )
    if tracked.returncode != 0:
        return None
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return None
    if line is None:
        line = 1
    elif line > len(lines):
        return None
    lo, hi = max(1, line - radius), min(len(lines), line + radius)
    return "\n".join(f"{n:>5}| {lines[n - 1]}" for n in range(lo, hi + 1))
