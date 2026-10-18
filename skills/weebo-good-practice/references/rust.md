# Rust: resilient and secure

Derived from the [ANSSI secure Rust guide](https://github.com/ANSSI-FR/rust-guide) (commit
`3f9e2e2`, Licence Ouverte 2.0); its rule IDs are in brackets. Where the guide says "MUST" and a
lint can check it, the lint is in `templates/lints.toml` and this file only says how to comply.

## 1. Panics

A function panics only when its caller broke its documented contract [LANG-LIMIT-PANIC].
Everything else returns `Result`/`Option`.

| Instead of | Write |
| --- | --- |
| `x.unwrap()` on input, I/O, parse, lookup | `x?` with context, or `x.ok_or(Error::Missing("key"))?` |
| `v[i]`, `&s[a..b]` [LANG-ARRINDEXING] | `v.get(i)`, `s.get(a..b)`, iterators, `split_at_checked`, `chunks_exact` |
| `a + b` on sizes, offsets, counters from input [LANG-ARITH] | `checked_add` (→ error), `saturating_add` (clamp is the right answer), `wrapping_add` (hashes, sequence numbers) |
| `as` between integer types | `u32::try_from(x)?`; `as` only widening, or with `#[expect(clippy::cast_possible_truncation, reason)]` |
| `Mutex::lock().unwrap()` | `.lock().unwrap_or_else(PoisonError::into_inner)` when the data stays valid, else map to an error |
| `unwrap()` in `main` | `fn main() -> Result<(), Error>` or `ExitCode` with a printed error |

**Justified panics** — the only accepted forms, all checked by the build:

```rust
// a compile-time-known value: literal regex, const URL, static table
#[expect(clippy::expect_used, reason = "literal pattern, test `re_compiles` pins it")]
fn re() -> &'static Regex { … .expect("literal regex") }

// a proven invariant, the proof one line away
#[expect(clippy::indexing_slicing, reason = "len checked == 4 on the line above")]
let tag = [buf[0], buf[1], buf[2], buf[3]];
```

Prefer `expect("why this holds")` to `unwrap()` even under `#[expect]`: the panic message is the
first thing on-call reads. Put the attribute on the smallest item or statement, never the module.

Tests unwrap freely (`templates/clippy.toml`). Test helpers outside `#[test]`/`#[cfg(test)]`
(e.g. `tests/common/mod.rs`) open with
`#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]`, listing
only the lints that file actually trips.

`Drop` never panics [LANG-DROP-NO-PANIC], and secret wiping never relies on `Drop` alone
[LANG-DROP-SEC]: use `zeroize` and wipe explicitly on the success path too.

## 2. Errors

- One error enum per library crate or module boundary, `thiserror`, implementing
  `Error + Send + Sync + 'static` [LANG-ERRWRAP]. Variants carry the context a reader needs
  (which file, which key), not a stringified source.
- `anyhow`/`eyre` only in binaries, scripts and tests: they erase the type a caller would match on.
- A `Result` is never discarded. `let _ =` on a `Result` is denied (`let_underscore_must_use`);
  when dropping is right, `if let Err(e) = … { tracing::warn!(error = %e, "why it is safe to go on") }`.
- Map errors at the boundary, once: internal detail (SQL, paths, stack) goes to the log, the HTTP
  response gets a stable code and a message that leaks nothing.

## 3. Untrusted input

- Parse, don't validate: turn input into a newtype whose constructor is the only check
  (`struct Slug(String)` with `TryFrom<&str>`). Fields stay private.
- Bound everything you allocate from input: body size, collection length, recursion depth, string
  length. `Vec::with_capacity(n)` with `n` from input is an allocation bomb — clamp `n` first.
- Paths from input: reject `..`, absolute paths and symlinks escaping the root; join, canonicalize,
  then check the prefix.
- Every parser of input it did not produce gets a fuzz target (`weebo-testing`).
- Secrets: never `Debug`/`Display` them (a `Secret<T>` wrapper, `secrecy`), never log them,
  compare with constant time (`subtle`).

## 4. `unsafe`, memory, FFI

- `unsafe_code = "deny"` workspace-wide. Code that needs it (FFI, a measured hot path) puts a
  reasoned `#[expect(unsafe_code, reason = "…")]` on the smallest item [LANG-UNSAFE]. Not
  `"forbid"`: forbid cannot be lifted, and a member crate cannot mix `workspace = true` with its
  own `[lints]`, so one FFI call would push the whole workspace off the shared table.
- Every `unsafe` block has a `// SAFETY:` comment naming the invariant, and sits behind a safe API
  no safe caller can misuse [LANG-UNSAFE-ENCP]. `unsafe fn` bodies still use explicit `unsafe {}`.
- No leaks: no `mem::forget` [MEM-FORGET], no `Box::leak` outside a one-shot `static` init
  [MEM-LEAK], no `Rc`/`Arc` cycles on `Drop` types [LANG-DROP-NO-CYCLE] — break them with `Weak`.
- FFI: wrap foreign functions in safe Rust [FFI-SAFEWRAPPING], use `std::ffi::c_*` types
  [FFI-PFTYPE], never let a panic cross the boundary [FFI-NOPANIC], generate bindings with
  `bindgen`/`cbindgen` rather than by hand [FFI-AUTOMATE].

## 5. Concurrency and async

- No blocking call or long CPU work in an async task: `spawn_blocking` (also in nr/rules).
- Every `.await` on the network has a timeout (`tokio::time::timeout` or the client's own).
- Channels bounded, spawned tasks tracked (`JoinSet`), shutdown explicit (`CancellationToken`):
  a task nobody joins hides its panic.
- No `Mutex` guard held across `.await`; `clippy::await_holding_lock` is on by default — keep it.
- Implementing `Send`/`Sync` by hand is `unsafe` and needs the §4 justification [LANG-SYNC-TRAITS].

## 6. Toolchain and dependencies

- Stable toolchain pinned in `rust-toolchain.toml` [DENV-STABLE]; nightly only for `cargo fuzz`.
- `Cargo.lock` committed for binaries [DENV-CARGO-LOCK].
- Never set `overflow-checks = false` or `debug-assertions = false` in `[profile.dev]`/`[profile.test]`
  [DENV-CARGO-OPTS].
- `cargo fmt` and clippy on every commit [DENV-FORMAT], [DENV-LINTER] — `task lint` in pre-commit.
- Each new crate is a choice [LIBS-VETTING-DIRECT]: the std or an existing dependency first
  (ponytail), `default-features = false` and only the features used, `cargo audit`/`cargo deny`
  clean [LIBS-AUDIT] (`weebo-security`). A crate with `unsafe` gets a look at its `unsafe` before
  it lands [LIBS-UNSAFE].
