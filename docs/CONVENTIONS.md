# Rust Conventions

## Toolchain and checks

- Stable Rust (`rust-toolchain.toml`), MSRV 1.82 (`rust-version` in the workspace manifest).
- Before every commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. All must be clean; clippy warnings are fixed, not allowed away.
- Edition 2021. Workspace lints: `unsafe_code = "forbid"`, `missing_docs = "warn"`, clippy `all = warn`. Integration-test files opt out of `missing_docs` with `#![allow(missing_docs)]`.

## Code style

- Public items are documented. Docs state behavior and limitations, not implementation trivia.
- Errors: recoverable input problems are `Diagnostic`s, not `Result`s or panics. API misuse that the caller can fix (e.g. tree edits) returns a typed error enum. Panics are for internal invariants only and never reachable from input.
- No `unwrap`/`expect` on data derived from input. `unwrap` on an invariant needs a comment or an obviously local proof.
- Prefer borrowing over cloning; take `&str`, return `&str` or `String` only when ownership is needed. Avoid per-token allocation in hot paths when a slice will do.
- Indices over pointers: tree structures use typed ids (`NodeId`), not `Rc<RefCell<_>>`.
- Keep functions short and modes/state machines table-like. A match arm that needs more than ~25 lines becomes a helper.
- Names follow the spec where one exists (`adoption_agency`, `generate_implied_end_tags`) so code can be audited against it.
- Dependencies: justify in the PR; prefer none. Pin through `Cargo.lock` for binaries; libraries use semver ranges.

## Unsupported and incomplete behavior

- Never silently ignore input. Emit `Diagnostic::unsupported(code, span)` and document the gap in ARCHITECTURE.md "Known limitations".
- Diagnostic codes are stable kebab-case strings; use the spec's error names where they exist.
- Do not claim conformance in docs or comments without a passing test suite cited.

## Testing

- Every behavior fix ships with a regression test that failed before the fix.
- Expected values come from the specification or a reference implementation, not from whatever the code currently outputs. If unsure, derive by hand against the spec and say so in the test.
- Any loop or recursion driven by input has a bound and a test at the bound.
- Property/fuzz-style tests must be seeded and deterministic.
- Tests must be fast: the whole workspace suite should stay under a few seconds in debug.

## Commits

- Small, buildable, tested commits. Subject in imperative mood, under 72 characters.
- Never delete or weaken a working test to make a change pass.
