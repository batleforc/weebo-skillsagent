import shutil
import subprocess
from pathlib import Path

import pytest

FIX = Path(__file__).parent / "fixtures"


@pytest.fixture
def ready_text() -> str:
    return (FIX / "rfc" / "ready.md").read_text()


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "thing.py").write_text(
        "max_entries = 1000\n\ndef load_config():\n    pass\n"
    )
    (tmp_path / "docs" / "rfc").mkdir(parents=True)
    shutil.copy(FIX / "rfc" / "ready.md", tmp_path / "docs" / "rfc" / "0001-cache-eviction.md")
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(["git", "add", "-A"], cwd=tmp_path, check=True)
    return tmp_path


def write_rfc(repo: Path, text: str, name: str = "0002-x.md") -> Path:
    p = repo / "docs" / "rfc" / name
    p.write_text(text)
    subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
    return p
