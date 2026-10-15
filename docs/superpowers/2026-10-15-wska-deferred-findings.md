# wska spine — known findings left unfixed

From the 13-task implementation of `docs/superpowers/specs/2026-10-15-skills-spine-design.md`.
Each was raised by a task review, judged non-blocking, and deliberately not fixed.
The final whole-branch review triaged all of them as safe to merge.

- **Task 1** — no `Signed-off-by:` trailer, against CLAUDE.md's "signed and signed-off by the user". See R8.
- **Task 1** — `Command::List` and `Command::Sync` ship untested in Task 1 (brief-mandated shape; covered in Tasks 10-11).
- **Task 2** — `mtime_change_does_not_change_hash` never asserts the mtime actually changed; the property holds trivially since the impl never reads mtime.
- **Task 2** — non-UTF8 / exotic path bytes untested (`as_encoded_bytes`); Linux-only scope makes this low risk.
- **Task 3** — `Source`/`Installer` fields trigger dead_code warnings until later tasks consume them. Harmless now; Task 12 turns on clippy `-D warnings`, but Task 9's lib.rs makes everything `pub`, which clears it. Watch at Task 12.
- **Task 3** — the non-NotFound IO branch (permission denied) has no test.
- **Task 4** — `metadata` is called per entry when `read_dir`'s `entry.file_type()` already carries type info on Linux — one redundant syscall per entry, pre-existing.
- **Task 5** — no test exercises the reserved-`all` collision when the caller asks for a DIFFERENT profile; the implementation is already correct for it (check is unconditional), so this is coverage only.
- **Task 6** — `user_scope_prefers_claude_config_dir` sets CLAUDE_CONFIG_DIR without a conflicting HOME, so it does not truly test precedence. Brief-verbatim, not implementer-introduced.
- **Task 7** — dead_code warnings for `points_to` fields and `target::repo_root`. Tasks 8/10 consume both, and Task 9's lib.rs makes items `pub`, which silences the lint — but verify at Task 12, which turns on clippy `-D warnings`.
- **Task 8** — `RunInstaller.cwd` at user scope resolves to the parent of `~/.claude`, which is meaningless; reachable only for a user-scoped installer, and `vendor.toml` declares only project-scoped ones. Revisit if a user-scoped installer is ever added.
- **Task 8** — the wanted-name-but-`Item::Installer` keep-guard combination has no dedicated test; logic is sound by inspection (`matches!` only matches `Item::Linked`).
- **Task 9** — `done.unlinked` increments even when `remove_if_link` removed nothing; unreachable via `plan::build`, which only emits Unlink for Owned* states.
- **Task 9** — `main.rs` uses `expect("repo root")` instead of the `error: {e}` + exit-1 path used elsewhere. Brief-specified.
- **Task 10** — `app::list`'s item branch scans with the raw `repo_root` rather than canonicalizing first, unlike `status`. Not a live bug — `main.rs` always passes an already-canonicalized root — but inconsistent with the discipline applied elsewhere.
- **Task 10** — `OwnedWrongTarget` remains unexercised in `status::collect`'s test; intentional per R5.
- **Task 11** — `.gitignore` gained `.task/wska-cache/` although `.task/` on line 2 already covers it — harmless redundancy, brief-specified.
- **Task 11** — a recorded hash of None with an existing local dir falls through to Patched and is never added to `fresh`, so a newly vendored skill that is byte-identical to upstream reports Patched permanently with no adopt path.
- **Task 11** — skills dropped from `provides` silently lose their recorded hash (safe direction — reappearing reads as None -> Patched).
- **Task 11** — a `Provides::Named` entry missing upstream aborts the whole source with an opaque "hashing" IO error.
- **Task 11** — the temp dir name `.wska-tmp-<skill>` is deterministic, so two concurrent syncs of the same skill could collide. Sync is not expected to run concurrently.
- **Task 11** — a leftover `.wska-tmp-*` from a crash is only cleared opportunistically on the next sync of that same skill, not swept generally.

## Larger items the final review raised

- **Dead code (~690 lines).** `sync.rs`, `hash.rs`, `config::{Source, Provides}`, `Command::Sync` and the two `skills:sync*` tasks are implemented and tested but unreachable, because no third-party skills are vendored. `task audit`'s licence step therefore cannot fail. Kept as documented future capability; delete it if vendoring is never adopted.
- **Apply is not atomic.** Installers run before unlinks, so a failing installer leaves links created and stale links not yet removed. Re-running converges.
- **`Applied.skipped` mixes two string kinds** — bare installer names and full "changed since planning" sentences. Only the printed count reads it today.
- **No end-to-end test of `--scope user`**, and no integration test that `install` refuses a *foreign symlink* (only a real directory is covered).
