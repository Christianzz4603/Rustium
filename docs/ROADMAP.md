# Rustium Roadmap

Phases follow the project brief. Each phase ends with: `cargo test --workspace` green, `cargo clippy` clean, docs updated, limitations listed. No dates are promised; a browser engine is a multi-year effort and this is a small project. Scope estimates are rough effort for one engineer.

## Phase 1: Workspace, HTML, DOM (in progress)

Done:
- [x] Cargo workspace, toolchain, lints (`unsafe` forbidden)
- [x] `rustium-common`, `rustium-dom`, `rustium-html`, `rustium-core`
- [x] Tokenizer, character references (subset), tree builder, adoption agency
- [x] Fragment parsing, parse diagnostics, depth and formatting-list limits
- [x] 120 tests including seeded fuzzing
- [x] html5lib tree-construction runner (`tools/html5lib-runner`); 74.8% of 1891 vectors pass (see CONFORMANCE.md)
- [x] Table insertion modes with foster parenting; in head noscript; script-data escape states; frameset modes; DOCTYPE identifiers and quirks mode; full named character reference table

Remaining before Phase 1 is closed:
- [x] Tokenizer vectors (`.test` JSON) in `tools/tokenizer-runner`; 99.6% pass (see CONFORMANCE.md)
- [x] Full named character reference table (2,125 names, generated from html5lib-tests namedEntities.test)
- [ ] In select, in template (template contents fragment), foreign content (svg/math)
- [x] Quirks mode detection from DOCTYPE (limited-quirks is not distinguished; it only affects layout)
- [x] Baseline parse benchmark in `benchmarks/` (throughput MB/s, node counts; peak memory not yet measured)
- [ ] DOM events infrastructure (design alongside JS bindings)
- [ ] Node reclamation / generation ids

## Phase 2: CSS and style

- `rustium-css`: tokenizer, parser (rules, at-rules skeleton, declarations), selectors Level 4 subset, specificity, value parsing (colors, lengths, keywords)
- `rustium-style`: selector matching, cascade (origin, specificity, order, `!important`), inheritance, computed values, default UA stylesheet
- Decide D3 (global interner)
- Invalidation hooks driven by DOM mutation records
- Compatibility target: CSS tests from WPT, selectors first

## Phase 3: Layout

- Box tree generation, block formatting, inline formatting, line breaking, text layout (needs `rustium-fonts` for metrics)
- Box model, overflow, scroll geometry, basic positioning (relative/absolute/fixed)
- Geometry independent of painting; layout cache and dirty bits

## Phase 4: Painting and software rendering

- Display list, software rasterizer (rectangles, borders, backgrounds, clipping, opacity, layering)
- Fonts (shaping, glyph rasterization), images (PNG, JPEG, GIF, WebP decoders; parallel decoding)
- Renderer trait (D6); first "render to PNG" CLI as the end-to-end demo
- Define the first stable-ish embedder API (D9)

## Phase 5: GPU, scrolling, incremental rendering

- `rustium-gpu` on wgpu behind the Renderer trait; software path stays as the reference and fallback
- Scrolling, compositing layers, partial repaint, incremental style and layout

## Phase 6: Networking and storage

- URL parsing (WHATWG), HTTP/1.1 and HTTP/2, TLS via rustls, redirects, compression, cache, connection reuse, timeouts and size limits
- Resource loader that never blocks rendering
- Decide D5 (isolation model) and D7 (async runtime) before starting
- Cookies, same-origin groundwork, `rustium-storage`

## Phase 7: JavaScript

- Lexer, parser (ES2015+ incrementally), AST, bytecode, interpreter, GC, builtins, Promises, event loop
- DOM bindings, console, timers, events, fetch/URL, storage, history/location
- Decide D4; if a third-party interpreter is used temporarily, document it and the replacement milestone
- Test262 as the conformance reference

## Phase 8: WebAssembly, security hardening, compatibility, optimization

- Wasm runtime (D8), sandboxed and resource-limited
- Origin isolation, CORS, CSP, secure contexts, permission model, certificate policy
- WPT-driven compatibility work; profile-guided optimization with the benchmark baselines from Phase 1

## Cross-cutting

- CI (build, test, clippy, fmt) on Linux, macOS, Windows; Android cross-compile check
- Platform abstraction traits before the first windowing code
- Fuzzing (`cargo-fuzz`) for tokenizer, parser, and later CSS/JS parsers
