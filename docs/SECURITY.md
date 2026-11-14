# Rustium Security

Status: Phase 1. Rustium currently only parses HTML into a DOM. It does no
networking, executes no script, and renders nothing, so most browser security
mechanisms do not exist yet. This document records what is in place, what is
planned, and the rules every change must follow.

## Reporting vulnerabilities

Open a private security advisory on the GitHub repository. Do not file public
issues for exploitable bugs.

## In place today

- `unsafe` is forbidden across the workspace (`unsafe_code = "forbid"`). Memory safety rests on the Rust compiler.
- The parser never panics on malformed input (covered by garbage-input tests and a seeded fuzz test, not by a proof).
- Resource limits against hostile markup:
  - Open-element depth cap (512) bounds tree depth and recursion.
  - Active formatting list: Noah's Ark clause, 256-entry cap, no reconstruction past the depth cap. A quadratic-blow-up bug (3000 `<b>` tags producing 6.2M nodes) was found and fixed; a regression test guards it.
  - Numeric character references saturate and map to U+FFFD outside Unicode range.
- Diagnostics report every recovered error and unsupported construct; nothing is dropped silently.

## Known gaps (do not rely on Rustium for untrusted content yet)

- No input size limit on `parse_document`; callers must cap input.
- The DOM arena never reclaims detached nodes (memory grows with mutation).
- Recursive serialization and tree dumps are bounded only by the depth cap; any new recursive tree walker must respect it or use an explicit stack.
- No fuzzing infrastructure beyond the seeded test (`cargo-fuzz` planned).

## Planned architecture (not implemented)

| Area | Plan |
|------|------|
| Origins | `Origin` type in `rustium-security`; every document, resource, and storage bucket carries one; same-origin checks are the only path to cross-origin data |
| Isolation | Decision D5 (process vs. thread isolation) is open and must be settled before networking work. Default assumption: renderer isolated per site, with a privileged browser process owning network, storage, and permissions |
| Sandbox | Renderer/JS/Wasm run without direct OS access; all I/O through an explicit, capability-style IPC surface |
| Network | rustls with certificate verification on by default, no silent downgrade, redirect and size limits, request timeouts |
| Policies | CORS, CSP, secure contexts, mixed-content blocking |
| JavaScript | Memory and time budgets, no host access except through audited bindings |
| WebAssembly | Isolated, fuel/memory-limited execution |
| Storage | Origin-partitioned; secrets encrypted at rest using platform keystores |
| Permissions | Per-origin grants, default deny |

## Rules for contributors

1. Never trade security for performance (project rule 10).
2. No `unsafe` without a design note in ARCHITECTURE.md and a dedicated, minimal crate.
3. Every input-driven loop and recursion needs a bound, and a test that exercises it.
4. New parsers (CSS, JS, URL, HTTP, image codecs) land with fuzz targets.
5. Do not claim a security property in docs or code comments without a test demonstrating it.
