"""judge: headless review gates. Exit 0 pass, 1 fail, 2 error, 3 unjudged."""

import argparse
import json
import subprocess
import sys
import traceback
from pathlib import Path

from judge import doc_review, rfc_ready
from judge import filter as filter_
from judge.backend import JudgeError, select_backend
from judge.report import exit_code, to_text


def _budget(text: str) -> float:
    v = float(text)
    if v <= 0:
        raise argparse.ArgumentTypeError("budget must be positive")
    return v


def _parser() -> argparse.ArgumentParser:
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--root", type=Path, help="target repo (default: git root of the cwd)")
    common.add_argument("--format", choices=("json", "text"), default="json")
    p = argparse.ArgumentParser(prog="judge", description=__doc__)
    sub = p.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("rfc-ready", parents=[common], help="is an RFC ready for sign-off")
    r.add_argument("files", nargs="*", type=Path)
    d = sub.add_parser("doc-review", parents=[common], help="is the change documented")
    scope = d.add_mutually_exclusive_group()
    scope.add_argument("--base", default="origin/main")
    scope.add_argument("--all", dest="whole", action="store_true")
    d.add_argument("--budget", type=_budget, default=1.0, help="max USD for the agent run")
    f = sub.add_parser("filter", parents=[common], help="filter a findings file through Jev")
    f.add_argument("--in", dest="infile", type=Path, required=True)
    return p


def _git_root() -> Path:
    out = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], check=False, capture_output=True, text=True
    )
    if out.returncode:
        raise JudgeError("not inside a git repo; pass --root")
    return Path(out.stdout.strip())


def _read_json(path: Path):
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as e:
        raise JudgeError(f"cannot read findings from {path}: {e}") from e


def main(argv=None) -> int:
    args = _parser().parse_args(argv)
    try:
        root = (args.root or _git_root()).resolve()
        if not root.is_dir():
            raise JudgeError(f"--root {root} is not a directory")
        backend = select_backend()
        if args.cmd == "rfc-ready":
            rep = rfc_ready.run(args.files, root, backend)
        elif args.cmd == "doc-review":
            rep = doc_review.run(root, args.base, args.whole, args.budget, backend)
        else:
            rep = filter_.run(filter_.parse_findings(_read_json(args.infile)), root, backend)
        # Rendered inside the try: a bug in formatting is exit 2 too, never a verdict.
        out = (
            json.dumps(rep, indent=2, ensure_ascii=False) if args.format == "json" else to_text(rep)
        )
        code = exit_code(rep)
    except JudgeError as e:
        print(f"judge: {e}", file=sys.stderr)
        return 2
    except Exception:  # noqa: BLE001 -- a crash is exit 2, never 1: no gate reads a bug as a verdict
        traceback.print_exc()
        return 2
    print(out)
    return code


def entry() -> None:
    sys.exit(main())
