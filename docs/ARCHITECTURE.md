# Rustium Architecture

Status: Phase 1. Sections marked **(planned)** describe intended design, not code.

## 1. Principles

1. Independent engine: no embedded browser engine, WebView, or outsourced core functionality.
2. Rust everywhere in the engine. Third-party crates only for foundations (TLS, async runtime, GPU abstraction, crypto, compression).
3. Each stage is a separate crate with a narrow API and its own tests.
4. Nothing is silently ignored. Unsupported or malformed input produces a `Diagnostic`.
5. `unsafe` is forbidden workspace-wide (`unsafe_code = "forbid"`). Any future exception needs a documented design decision and a dedicated crate.
6. Measure before optimizing. Benchmarks and baselines precede any non-trivial optimization.

## 2. Pipeline

```
bytes -> [net] -> HTML tokenizer -> tree builder -> DOM
                                                     |
CSS text -> tokenizer -> parser -> stylesheet --+    |
                                                v    v
                                         style (selector match, cascade,
                                                inheritance, computed styles)
                                                     |
                                                  layout tree -> geometry
                                                     |
                                                 display list
                                                     |
                                            renderer (software, then GPU)
```

Each arrow is a crate boundary. Layout produces geometry without knowing about
painting; style knows nothing about layout.

## 3. Crates

### Implemented

| Crate | Responsibility | Depends on |
|-------|----------------|-----------|
| `rustium-common` | `Atom` (static or ref-counted string), `Span`, `Diagnostic`/`Severity` | none |
| `rustium-dom` | Arena DOM: nodes, tree edits, attributes, traversal, simple queries, mutation records, serialization | common |
| `rustium-html` | Tokenizer, character references, tree builder, document and fragment parsing | common, dom |
| `rustium-core` | Facade for embedders (`Document::parse`); re-exports subsystem crates | common, dom, html |

### Planned (created only when the first real code lands)

| Crate | Phase | Responsibility |
|-------|-------|----------------|
| `rustium-css` | 2 | CSS tokenizer, parser, selectors, specificity, values |
| `rustium-style` | 2 | Selector matching, cascade, inheritance, computed styles, invalidation |
| `rustium-fonts` | 3-4 | Font loading, shaping, metrics |
| `rustium-layout` | 3 | Box tree, block/inline/text layout, geometry |
| `rustium-paint` | 4 | Display list construction |
| `rustium-renderer` | 4 | Software rasterizer behind a `Renderer` trait |
| `rustium-images` | 4 | Image decoding |
| `rustium-gpu` | 5 | wgpu-based renderer implementing the same trait |
| `rustium-net` | 6 | URL, HTTP/1.1+2, TLS (rustls), cache, resource loader |
| `rustium-js` | 7 | Lexer, parser, AST, bytecode, interpreter, GC, builtins |
| `rustium-storage`, `rustium-security`, `rustium-wasm`, `rustium-extensions` | 6-8 | Created when there is code to put in them |

The original brief lists all of these up front; they are deferred on purpose
to avoid empty crates (brief section 3).

### Dependency rules

- Dependencies point downward only: `core -> {html, style, layout, ...} -> dom -> common`.
- No crate depends on `rustium-core`.
- `rustium-dom` knows nothing about HTML parsing, CSS, or layout.
- Platform code (Android, Linux, Windows, macOS) lives outside engine crates behind traits (windowing, clipboard, file access). The engine crates stay `std`-only and platform-agnostic.

## 4. DOM design

- Nodes live in one `Vec<Node>`; `NodeId(u32)` is the handle. Links (`parent`, `first_child`, `last_child`, `prev_sibling`, `next_sibling`) are `Option<NodeId>`. No `Rc<RefCell<_>>`.
- Handles suit future JS bindings: a wrapper object can hold `(DomId, NodeId)`.
- Insertion rejects cycles and non-container parents; inserting a fragment moves its children.
- Element names and attribute names are `Atom`s (cheap clone). Text and attribute values are `String`.
- Mutation recording is opt-in (`set_mutation_recording`) and records inserts, removals, attribute and text changes. This is the hook for incremental style/layout invalidation.
- Detached nodes are not reclaimed until the `Dom` drops. A free list or generation-tagged ids are needed before long-lived mutation-heavy pages (JS phase).
- Events: infrastructure is **not implemented** yet; it depends on the JS binding design.

## 5. HTML parsing design

- Input is newline-normalized (CRLF/CR to LF). Spans refer to normalized text.
- The tokenizer is a pull-based hand-written scanner (not regex). It switches content model itself after `title`, `textarea`, `style`, `script`, `xmp`, `iframe`, `noembed`, `noframes`, `noscript` (when scripting is on), and `plaintext` start tags.
- The tree builder implements the spec insertion modes listed in `tree_builder.rs`, the active formatting list, and the adoption agency algorithm.
- Scripting defaults to **enabled** (`ParseOptions::scripting`), matching a JS-capable browser, so `<noscript>` content is raw text.
- Resource limits:
  - Open-element depth is capped at 512 (`MAX_OPEN_DEPTH`); deeper elements attach to the current node but are not pushed, which bounds recursion in serialization and later stages. Reported as `max-open-element-depth-exceeded`.
  - The active formatting list applies the Noah's Ark clause and a hard cap of 256 entries. Reconstruction is skipped past the depth cap. This fixed a quadratic blow-up found during testing (3000 unclosed `<b>` produced 6.2M elements).

## 6. Critical decisions

| # | Decision | Status |
|---|----------|--------|
| D1 | Arena DOM with `u32` ids instead of `Rc<RefCell>` | Made |
| D2 | `unsafe` forbidden for now | Made |
| D3 | `Atom` is a static-or-`Rc<str>` string (common HTML names do not allocate); global interner later | Open: decide at Phase 2 when selector matching needs fast name compare |
| D4 | JS engine: own interpreter (Phase 7). A third-party Rust JS engine may be used temporarily; it must be documented here with a replacement plan | Open |
| D5 | Multi-process vs. multi-thread isolation model | Open: decide before Phase 6; affects `rustium-security` and the embedder API |
| D6 | Renderer trait boundary between display list and backends | Open: Phase 4 |
| D7 | Async runtime for networking (Tokio) and TLS (rustls) | Open: Phase 6 |
| D8 | WebAssembly runtime (Wasmtime evaluation) | Open: Phase 8. Note Wasmtime is a large mostly-non-Rust-owned dependency; evaluate against the "no outsourced core functionality" rule |
| D9 | Stable embedder API shape for Axylum | Open: after Phase 4; `rustium-core` is unstable until then |

## 7. Known limitations (HTML/DOM)

Each is reported at runtime as an `Unsupported` diagnostic where practical.

- Table modes (in table, table text, caption, column group, table body, row, cell) and foster parenting follow the spec, but `<template>` interactions and the select-in-table modes are not implemented.
- No "in select" or "in template" modes. `<template>` has no content fragment. The frameset modes and the `frameset-ok` flag are implemented (`frameset.rs`); `frameset-ok` is cleared by an explicit list of start tags and by non-whitespace text.
- No foreign content: `<svg>` and `<math>` are plain HTML elements.
- Quirks mode follows the spec's DOCTYPE rules (`quirks.rs`), but limited-quirks mode is not distinguished from no-quirks because it only affects layout. In the parser, quirks mode only affects whether `<table>` closes an open `<p>`. Fragment parsing does not take a quirks mode from the context.
- Named character references: the full table (2,125 names; 106 legacy names also work without `;`) is generated from html5lib-tests `namedEntities.test` into `entity_table.rs`. It is not re-verified against the WHATWG `entities.json` directly.
- Processing instructions: some current html5lib/WPT vectors (`processing-instructions.dat`) expect `<?x>` to become a processing-instruction node; Rustium emits a bogus comment as the HTML Living Standard version it was written against does. Not verified against the latest spec text.
- NUL in data state is dropped (the spec emits it and the tree builder ignores it; the visible result is the same).
- Serializer escapes all text, including inside `<script>`/`<style>`; it is a debug aid, not the spec's fragment serialization algorithm.
- DOM events, live collections, namespaces, shadow DOM, node reclamation: not implemented.
- Measured against the WPT html5lib tree-construction vectors: 1414 of 1891 run pass (74.8%), 67 skipped. This is a measurement, not a compliance claim; see `docs/CONFORMANCE.md` for the failing areas. Correctness evidence also includes 119 hand-written tests and a seeded fuzz test that checks tree invariants.

## 8. Testing strategy

- Unit tests live beside code; integration tests in each crate's `tests/`.
- Tree-construction expectations are written as serialized HTML or tree dumps.
- A seeded fuzz test (`seeded_fuzz_keeps_tree_invariants`) parses 3000 generated inputs as documents and fragments and verifies sibling/parent link consistency.
- Tree-construction vectors from web-platform-tests run through `tools/html5lib-runner` (not vendored; see `docs/CONFORMANCE.md`). Next: tokenizer `.test` vectors (needs a JSON reader), then foreign content, `<template>`, and DOCTYPE identifiers.
