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
- Parse diagnostics: every recovered error and every unsupported construct is reported, never silently dropped

What does **not** exist yet: CSS, style, layout, painting, networking, JavaScript,
WebAssembly, security boundaries. See [docs/ROADMAP.md](docs/ROADMAP.md).

**No standards-compliance claim is made.** The parser has not yet been run
against html5lib-tests or the Web Platform Tests; see "Known limitations" in
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
| `crates/rustium-core` | Public facade for embedders (API unstable) |
| `docs/` | Architecture, roadmap, security, conventions |

## License

MIT. See [LICENSE](LICENSE).
