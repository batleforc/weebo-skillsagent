# Fuzzing (cargo-fuzz + libFuzzer)

**Trigger:** the change adds or modifies code that parses or validates bytes it did not produce:
paths, archives, manifests, version strings, URLs and IPs (SSRF), signed URLs, sanitisers, policy
evaluators. Rule: **every new parser gets a fuzz target in the same change.**

## Layout

```
fuzz/
├── Cargo.toml                 own [workspace]; one [[bin]] per target, test = false, doc = false
├── fuzz_targets/fuzz_<thing>.rs
└── dicts/fuzz_<thing>.dict    optional libFuzzer dictionary
```

`fuzz/corpus/`, `fuzz/artifacts/` and `fuzz/target/` are gitignored. CI caches the corpus instead of
committing it.

`fuzz/Cargo.toml` depends on the project crates with `default-features = false` and only the
parser features enabled: the fuzz build carries no network client and no database driver.

## Writing a target

Template: `templates/fuzz/fuzz_targets/fuzz_example.rs`.

- **Assert an invariant, not just "no panic".** "No panic" is the floor. The value comes from a
  property: the round trip `parse(render(x)) == x`, or "the guard accepts ⇒ no decoding depth
  reveals `..`".
- **Write the oracle independently.** When the target checks a guard, re-implement the property
  in the target (batlehub's `fuzz_path_safe` has its own `%XX` decoder) and share no code with the
  thing under test.
- **Structured input:** `arbitrary::Unstructured::new(data)` and `u.arbitrary::<T>()`, or
  `#[derive(Arbitrary)]` on an input struct. `let Ok(x) = … else { return };` drops inputs that
  cannot be decoded.
- **Dictionary:** when the format has keywords (tags, schemes, field names), write them in
  `dicts/<target>.dict`, one quoted token per line. A byte mutator would take hours to spell them.

## Running

- `task fuzz TARGET=fuzz_example MAX_TIME=30` runs locally (nightly; `cargo fuzz list` names
  the targets).
- `task fuzz:check` checks that every target compiles (stable). This is the per-PR gate.
- Reproduce a crash: `cargo +nightly fuzz run <target> fuzz/artifacts/<target>/crash-…`. Then add
  that input as a regular unit test beside the parser before fixing it.

## Two gotchas, already handled in the templates

- `--target "$(rustc +nightly -vV | sed -n 's/^host: //p')"`: cargo-fuzz otherwise defaults to the
  triple it was *built* for. A prebuilt cargo-fuzz is static musl, which cannot carry
  AddressSanitizer.
- `ASAN_OPTIONS=detect_leaks=0`: LeakSanitizer cannot start under a restricted `ptrace_scope`
  (CI runners, CDE pods). It reports the refusal as a 0-byte `crash-da39a3ee…` file that looks
  exactly like a finding.

## CI (two tiers)

1. **Every PR:** `cargo check --manifest-path fuzz/Cargo.toml --bins --all-targets`. When batlehub
   added this gate, four of its six targets had silently stopped compiling.
2. **Nightly and manual dispatch:** for every target in `cargo fuzz list`,
   `-max_total_time=60` with its dictionary, then `cargo fuzz cmin`. The corpus is restored from and
   saved to the Actions cache, and saved even on failure. Crash artifacts are uploaded. Failures are
   collected and the job fails once, naming every failing target.
