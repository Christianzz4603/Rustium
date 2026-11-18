# Rustium

An independent, Rust-native web browser engine. Rustium is not a wrapper around
Chromium, Gecko, WebKit, Servo, CEF, Electron, or any system WebView: the HTML
parser, DOM, and (eventually) CSS, layout, painting, and JavaScript runtime are
written from scratch in this repository.

Rustium is developed first, on its own. Its intended first consumer is the
Axylum browser application, which is **not** part of this repository.

## Status: early (Phase 1)

What works today:

- HTML tokenizer (data, RCDATA, RAWTEXT, script data, PLAINTEXT, comments, DOCTYPE, character references)
- HTML tree builder with misnested-markup recovery (adoption agency algorithm, implied end tags)
- Document and fragment parsing
- Arena-backed DOM with tree mutation, attributes, traversal, simple queries, mutation records, serialization
- CSS tokenizer (CSS Syntax Level 3 tokenization, with spec parse errors recorded); tokenizer only so far
- Parse diagnostics: every recovered error and every unsupported construct is reported, never silently dropped

What does **not** exist yet: a CSS parser, selectors, style, layout, painting, networking, JavaScript,
WebAssembly, security boundaries. See [docs/ROADMAP.md](docs/ROADMAP.md).

**No standards-compliance claim is made.** Measured against the web-platform-tests
html5lib tree-construction vectors, 75.6% pass (1430 of 1891 run, 67 skipped);
the html5lib tokenizer vectors pass 99.6% (6951 of 6980 run, 66 skipped);
see [docs/CONFORMANCE.md](docs/CONFORMANCE.md) and the known limitations in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Use

```rust
let doc = rustium_core::Document::parse("<p>Hello <b>world</b>");
assert_eq!(
    doc.to_html(),
    "<html><head></head><body><p>Hello <b>world</b></p></body></html>"
);
```

## Build and test

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
```

The engine currently has no third-party dependencies.

## Layout

| Path | Purpose |
|------|---------|
| `crates/rustium-common` | Shared types: `Atom`, `Span`, `Diagnostic` |
| `crates/rustium-dom` | Arena DOM |
| `crates/rustium-html` | Tokenizer and tree builder |
| `crates/rustium-css` | CSS tokenizer (parser not started) |
| `crates/rustium-core` | Public facade for embedders (API unstable) |
| `tools/html5lib-runner` | Runs html5lib tree-construction vectors and reports pass rates |
| `tools/tokenizer-runner` | Runs html5lib tokenizer vectors and reports pass rates |
| `benchmarks/` | Parse benchmark |
| `docs/` | Architecture, conformance, roadmap, security, conventions |

## License

Free to use, change and ship, even commercially, under the [Rustium License](LICENSE): an MIT-based license
with two human-readable extras. Products built on Rustium must credit the author (Christian Jay,
Christianzz4603), and the author isn't responsible for what others build with it. This is a plain-language
summary, not legal advice; the LICENSE text is what applies.
