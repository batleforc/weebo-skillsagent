import subprocess

from judge.excerpt import excerpt


def test_excerpt_window_is_numbered_and_clamped(repo):
    got = excerpt(repo, "src/thing.py", 1, radius=1)
    assert got.splitlines() == ["    1| max_entries = 1000", "    2| "]


def test_no_line_means_the_head_of_the_file(repo):
    assert excerpt(repo, "src/thing.py", None).startswith("    1| max_entries")


def test_line_past_eof_or_missing_file_is_none(repo):
    assert excerpt(repo, "src/thing.py", 999) is None
    assert excerpt(repo, "src/nope.py", 1) is None


def test_excerpt_refuses_untracked_and_outside_paths(repo, tmp_path_factory):
    (repo / ".env").write_text("TOKEN=secret\n")
    (repo / ".gitignore").write_text(".env\n")
    subprocess.run(["git", "add", ".gitignore"], cwd=repo, check=True)
    outside = tmp_path_factory.mktemp("out") / "x.txt"
    outside.write_text("x\n")
    for path in [".env", str(outside), "../x.txt", ".git/config", "/etc/passwd"]:
        assert excerpt(repo, path, 1) is None, path
