//! HTML tokenizer.
//!
//! A hand-written tokenizer following the HTML Living Standard's tokenization
//! stage, restructured around helper functions instead of one state per enum
//! variant. Implemented: data, RCDATA, RAWTEXT, script data (including the
//! escaped and double-escaped sub-states), PLAINTEXT, tags, attributes,
//! comments, bogus comments, DOCTYPEs (with public/system identifiers), and
//! character references.
//!
//! Not implemented: CDATA sections in foreign content (no foreign content yet).
//!
//! Input must already have CRLF/CR normalized to LF (see `lib.rs`).

use crate::entities::decode_char_ref;
use crate::token::{SpannedToken, Token};
use rustium_common::{Diagnostic, Span};

/// Content model the tokenizer switches to after certain start tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawKind {
    /// `<title>`, `<textarea>`: text with character references.
    RcData,
    /// `<style>`, `<xmp>`, `<iframe>`, `<noembed>`, `<noframes>`, `<noscript>` (scripting on).
    RawText,
    /// `<script>`.
    Script,
    /// `<plaintext>`: everything until EOF.
    Plaintext,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DoctypeKind {
    Public,
    System,
}

/// Where the DOCTYPE state machine continues after an identifier or keyword.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DoctypeNext {
    Emit,
    Bogus,
    AfterPublic,
    AfterSystem,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RefCtx {
    Data,
    RcData,
    Attr,
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0c)
}

/// Returns the raw content model implied by a start tag name, if any.
pub fn raw_kind_for(name: &str, scripting: bool) -> Option<RawKind> {
    match name {
        "title" | "textarea" => Some(RawKind::RcData),
        "style" | "xmp" | "iframe" | "noembed" | "noframes" => Some(RawKind::RawText),
        "noscript" if scripting => Some(RawKind::RawText),
        "script" => Some(RawKind::Script),
        "plaintext" => Some(RawKind::Plaintext),
        _ => None,
    }
}

/// The HTML tokenizer.
pub struct Tokenizer<'a> {
    src: &'a str,
    pos: usize,
    raw: Option<(RawKind, String)>,
    scripting: bool,
    diags: Vec<Diagnostic>,
}

impl<'a> Tokenizer<'a> {
    /// Creates a tokenizer over newline-normalized input.
    pub fn new(src: &'a str, scripting: bool) -> Self {
        Tokenizer {
            src,
            pos: 0,
            raw: None,
            scripting,
            diags: Vec::new(),
        }
    }

    /// Starts in a raw content model (used for fragment parsing contexts).
    pub fn set_raw(&mut self, kind: RawKind, tag: &str) {
        self.raw = Some((kind, tag.to_string()));
    }

    /// Takes accumulated parse errors.
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.diags)
    }

    /// Produces the next token; returns `Eof` forever once input is exhausted.
    pub fn next_token(&mut self) -> SpannedToken {
        loop {
            let start = self.pos;
            if self.pos >= self.src.len() {
                return SpannedToken {
                    token: Token::Eof,
                    span: Span { start, end: start },
                };
            }
            let tok = match self.raw.clone() {
                Some((kind, name)) => self.lex_raw(kind, &name),
                None => self.lex_data(),
            };
            if let Some(token) = tok {
                return SpannedToken {
                    token,
                    span: Span {
                        start,
                        end: self.pos,
                    },
                };
            }
        }
    }

    fn diag(&mut self, code: &'static str) {
        let at = self.pos;
        self.diags
            .push(Diagnostic::parse_error(code, Span { start: at, end: at }));
    }

    fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(is_ws) {
            self.pos += 1;
        }
    }

    fn take_until(&mut self, pred: impl Fn(u8) -> bool) -> &'a str {
        let src = self.src;
        let b = src.as_bytes();
        let start = self.pos;
        let mut i = start;
        while i < b.len() && !pred(b[i]) {
            i += 1;
        }
        self.pos = i;
        &src[start..i]
    }

    /// Decodes character references and NULs in `s`, which begins at `base`.
    fn decode_text(&mut self, s: &str, base: usize, ctx: RefCtx) -> String {
        let b = s.as_bytes();
        let mut out = String::with_capacity(s.len());
        let (mut i, mut last) = (0, 0);
        while i < b.len() {
            match b[i] {
                b'&' => {
                    if let Some(r) = decode_char_ref(&s[i + 1..], ctx == RefCtx::Attr) {
                        out.push_str(&s[last..i]);
                        out.push_str(&r.value);
                        if let Some(code) = r.error {
                            let at = base + i;
                            self.diags
                                .push(Diagnostic::parse_error(code, Span { start: at, end: at }));
                        }
                        i += 1 + r.consumed;
                        last = i;
                    } else {
                        i += 1;
                    }
                }
                0 => {
                    out.push_str(&s[last..i]);
                    if ctx != RefCtx::Data {
                        out.push('\u{FFFD}');
                    }
                    let at = base + i;
                    self.diags.push(Diagnostic::parse_error(
                        "unexpected-null-character",
                        Span { start: at, end: at },
                    ));
                    i += 1;
                    last = i;
                }
                _ => i += 1,
            }
        }
        out.push_str(&s[last..]);
        out
    }

    fn begins_construct(&self, at: usize) -> bool {
        matches!(self.src.as_bytes().get(at), Some(c) if c.is_ascii_alphabetic() || matches!(c, b'/' | b'!' | b'?'))
    }

    fn lex_data(&mut self) -> Option<Token> {
        let b = self.src.as_bytes();
        let mut i = self.pos;
        while i < b.len() {
            if b[i] == b'<' {
                if self.begins_construct(i + 1) {
                    break;
                }
                let at = i;
                self.diags.push(Diagnostic::parse_error(
                    "invalid-first-character-of-tag-name",
                    Span { start: at, end: at },
                ));
            }
            i += 1;
        }
        if i > self.pos {
            let src = self.src;
            let base = self.pos;
            self.pos = i;
            let text = self.decode_text(&src[base..i], base, RefCtx::Data);
            return if text.is_empty() {
                None
            } else {
                Some(Token::Character(text))
            };
        }
        self.lex_markup()
    }

    /// `pos` is at a `<` that begins a construct.
    fn lex_markup(&mut self) -> Option<Token> {
        match self.src.as_bytes()[self.pos + 1] {
            b'!' => Some(self.lex_markup_declaration()),
            b'/' => {
                self.pos += 2;
                match self.peek() {
                    None => {
                        self.diag("eof-before-tag-name");
                        Some(Token::Character("</".to_string()))
                    }
                    Some(c) if c.is_ascii_alphabetic() => self.lex_tag(true),
                    Some(b'>') => {
                        self.diag("missing-end-tag-name");
                        self.pos += 1;
                        None
                    }
                    Some(_) => {
                        self.diag("invalid-first-character-of-tag-name");
                        Some(self.lex_bogus_comment())
                    }
                }
            }
            b'?' => {
                self.pos += 1;
                self.diag("unexpected-question-mark-instead-of-tag-name");
                Some(self.lex_bogus_comment())
            }
            _ => {
                self.pos += 1;
                self.lex_tag(false)
            }
        }
    }

    /// `pos` is at `<!`.
    fn lex_markup_declaration(&mut self) -> Token {
        let rest = &self.src[self.pos + 2..];
        if rest.starts_with("--") {
            self.pos += 4;
            self.lex_comment()
        } else if rest.len() >= 7 && rest[..7].eq_ignore_ascii_case("doctype") {
            self.pos += 9;
            self.lex_doctype()
        } else if rest.starts_with("[CDATA[") {
            self.diag("cdata-in-html-content");
            self.pos += 2;
            self.lex_bogus_comment()
        } else {
            self.diag("incorrectly-opened-comment");
            self.pos += 2;
            self.lex_bogus_comment()
        }
    }

    fn lex_bogus_comment(&mut self) -> Token {
        let content = self.take_until(|b| b == b'>');
        if self.peek().is_some() {
            self.pos += 1;
        }
        Token::Comment(content.replace('\0', "\u{FFFD}"))
    }

    /// `pos` is just after `<!--`.
    fn lex_comment(&mut self) -> Token {
        let rest = &self.src[self.pos..];
        if rest.starts_with('>') {
            self.pos += 1;
            self.diag("abrupt-closing-of-empty-comment");
            return Token::Comment(String::new());
        }
        if rest.starts_with("->") {
            self.pos += 2;
            self.diag("abrupt-closing-of-empty-comment");
            return Token::Comment(String::new());
        }
        let a = rest.find("-->").map(|i| (i, 3, false));
        // Only look for `--!>` before the `-->` terminator; scanning the whole
        // remainder for every comment made tokenizing quadratic.
        let limit = a.map_or(rest.len(), |(i, _, _)| (i + 3).min(rest.len()));
        let b = rest[..limit].find("--!>").map(|i| (i, 4, true));
        let end = match (a, b) {
            (Some(x), Some(y)) => Some(if y.0 < x.0 { y } else { x }),
            (x, None) => x,
            (None, y) => y,
        };
        let (content, advance) = match end {
            Some((i, len, bad)) => {
                if bad {
                    self.pos += i;
                    self.diag("incorrectly-closed-comment");
                    self.pos -= i;
                }
                (&rest[..i], i + len)
            }
            None => {
                self.pos += rest.len();
                self.diag("eof-in-comment");
                self.pos -= rest.len();
                // At EOF the spec's end-dash/end/end-bang states have not yet
                // appended their pending `-`, `--` or `--!` to the comment.
                let keep = if rest.ends_with("--!") {
                    rest.len() - 3
                } else if rest.ends_with("--") {
                    rest.len() - 2
                } else if rest.ends_with('-') {
                    rest.len() - 1
                } else {
                    rest.len()
                };
                (&rest[..keep], rest.len())
            }
        };
        self.pos += advance;
        Token::Comment(content.replace('\0', "\u{FFFD}"))
    }

    /// `pos` is just after `<!DOCTYPE`. Implements the spec's DOCTYPE states,
    /// including PUBLIC/SYSTEM identifiers, force-quirks, and bogus DOCTYPE.
    fn lex_doctype(&mut self) -> Token {
        let mut force_quirks = false;
        self.skip_ws();
        let name = match self.peek() {
            None => {
                self.diag("eof-in-doctype");
                return Token::Doctype {
                    name: None,
                    public_id: None,
                    system_id: None,
                    force_quirks: true,
                };
            }
            Some(b'>') => {
                self.diag("missing-doctype-name");
                self.pos += 1;
                return Token::Doctype {
                    name: None,
                    public_id: None,
                    system_id: None,
                    force_quirks: true,
                };
            }
            Some(_) => {
                let raw = self.take_until(|b| is_ws(b) || b == b'>');
                Some(lower_name(raw))
            }
        };
        let mut public_id = None;
        let mut system_id = None;
        self.skip_ws();
        let mut next = match self.peek() {
            None => {
                self.diag("eof-in-doctype");
                force_quirks = true;
                DoctypeNext::Emit
            }
            Some(b'>') => {
                self.pos += 1;
                DoctypeNext::Emit
            }
            Some(_) => {
                let rest = self.src.as_bytes();
                let kw = |k: &[u8]| {
                    rest.len() >= self.pos + 6
                        && rest[self.pos..self.pos + 6].eq_ignore_ascii_case(k)
                };
                if kw(b"public") {
                    self.pos += 6;
                    self.doctype_keyword(DoctypeKind::Public, &mut public_id, &mut force_quirks)
                } else if kw(b"system") {
                    self.pos += 6;
                    self.doctype_keyword(DoctypeKind::System, &mut system_id, &mut force_quirks)
                } else {
                    self.diag("invalid-character-sequence-after-doctype-name");
                    force_quirks = true;
                    DoctypeNext::Bogus
                }
            }
        };
        // After a PUBLIC identifier: optional SYSTEM identifier.
        if next == DoctypeNext::AfterPublic {
            next = self.doctype_after_public(&mut system_id, &mut force_quirks);
        }
        if next == DoctypeNext::AfterSystem {
            self.skip_ws();
            next = match self.peek() {
                None => {
                    self.diag("eof-in-doctype");
                    force_quirks = true;
                    DoctypeNext::Emit
                }
                Some(b'>') => {
                    self.pos += 1;
                    DoctypeNext::Emit
                }
                Some(_) => {
                    self.diag("unexpected-character-after-doctype-system-identifier");
                    DoctypeNext::Bogus
                }
            };
        }
        if next == DoctypeNext::Bogus {
            self.take_until(|b| b == b'>');
            if self.peek().is_some() {
                self.pos += 1;
            }
        }
        Token::Doctype {
            name,
            public_id,
            system_id,
            force_quirks,
        }
    }

    /// Reads a quoted DOCTYPE identifier whose opening quote is at `pos`.
    /// Returns the next state: `Done` when the closing quote was found.
    fn doctype_quoted(
        &mut self,
        kind: DoctypeKind,
        slot: &mut Option<String>,
        force_quirks: &mut bool,
    ) -> DoctypeNext {
        let q = self.src.as_bytes()[self.pos];
        self.pos += 1;
        let base = self.pos;
        let raw = self.take_until(|b| b == q || b == b'>');
        if raw.contains('\0') {
            self.diags.push(Diagnostic::parse_error(
                "unexpected-null-character",
                Span {
                    start: base,
                    end: base,
                },
            ));
        }
        *slot = Some(raw.replace('\0', "\u{FFFD}"));
        match self.peek() {
            Some(c) if c == q => {
                self.pos += 1;
                match kind {
                    DoctypeKind::Public => DoctypeNext::AfterPublic,
                    DoctypeKind::System => DoctypeNext::AfterSystem,
                }
            }
            Some(_) => {
                self.diag(match kind {
                    DoctypeKind::Public => "abrupt-doctype-public-identifier",
                    DoctypeKind::System => "abrupt-doctype-system-identifier",
                });
                *force_quirks = true;
                self.pos += 1;
                DoctypeNext::Emit
            }
            None => {
                self.diag("eof-in-doctype");
                *force_quirks = true;
                DoctypeNext::Emit
            }
        }
    }

    /// After the PUBLIC or SYSTEM keyword, up to and including the identifier.
    fn doctype_keyword(
        &mut self,
        kind: DoctypeKind,
        slot: &mut Option<String>,
        force_quirks: &mut bool,
    ) -> DoctypeNext {
        let (miss_ws, miss_id, miss_quote) = match kind {
            DoctypeKind::Public => (
                "missing-whitespace-after-doctype-public-keyword",
                "missing-doctype-public-identifier",
                "missing-quote-before-doctype-public-identifier",
            ),
            DoctypeKind::System => (
                "missing-whitespace-after-doctype-system-keyword",
                "missing-doctype-system-identifier",
                "missing-quote-before-doctype-system-identifier",
            ),
        };
        let had_ws = self.peek().is_some_and(is_ws);
        self.skip_ws();
        match self.peek() {
            Some(b'"' | b'\'') => {
                if !had_ws {
                    self.diag(miss_ws);
                }
                self.doctype_quoted(kind, slot, force_quirks)
            }
            Some(b'>') => {
                self.diag(miss_id);
                *force_quirks = true;
                self.pos += 1;
                DoctypeNext::Emit
            }
            None => {
                self.diag("eof-in-doctype");
                *force_quirks = true;
                DoctypeNext::Emit
            }
            Some(_) => {
                self.diag(miss_quote);
                *force_quirks = true;
                DoctypeNext::Bogus
            }
        }
    }

    /// After a closing quote of the public identifier: an optional system identifier.
    fn doctype_after_public(
        &mut self,
        system_id: &mut Option<String>,
        force_quirks: &mut bool,
    ) -> DoctypeNext {
        let had_ws = self.peek().is_some_and(is_ws);
        self.skip_ws();
        match self.peek() {
            Some(b'>') => {
                self.pos += 1;
                DoctypeNext::Emit
            }
            Some(b'"' | b'\'') => {
                if !had_ws {
                    self.diag("missing-whitespace-between-doctype-public-and-system-identifiers");
                }
                self.doctype_quoted(DoctypeKind::System, system_id, force_quirks)
            }
            None => {
                self.diag("eof-in-doctype");
                *force_quirks = true;
                DoctypeNext::Emit
            }
            Some(_) => {
                self.diag("missing-quote-before-doctype-system-identifier");
                *force_quirks = true;
                DoctypeNext::Bogus
            }
        }
    }

    /// `pos` is at the first character of a tag name (an ASCII letter).
    fn lex_tag(&mut self, is_end: bool) -> Option<Token> {
        let raw_name = self.take_until(|b| is_ws(b) || b == b'/' || b == b'>');
        let name = lower_name(raw_name);
        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut self_closing = false;
        loop {
            self.skip_ws();
            match self.peek() {
                None => {
                    self.diag("eof-in-tag");
                    return None;
                }
                Some(b'>') => {
                    self.pos += 1;
                    break;
                }
                Some(b'/') => {
                    self.pos += 1;
                    if self.peek() == Some(b'>') {
                        self.pos += 1;
                        self_closing = true;
                        break;
                    }
                    self.diag("unexpected-solidus-in-tag");
                    continue;
                }
                Some(_) => {}
            }
            let name_start = self.pos;
            if self.peek() == Some(b'=') {
                self.diag("unexpected-equals-sign-before-attribute-name");
                self.pos += 1;
            }
            self.take_until(|b| is_ws(b) || matches!(b, b'/' | b'>' | b'='));
            let aname = lower_name(&self.src[name_start..self.pos]);
            self.skip_ws();
            let mut value = String::new();
            if self.peek() == Some(b'=') {
                self.pos += 1;
                self.skip_ws();
                match self.peek() {
                    Some(q @ (b'"' | b'\'')) => {
                        self.pos += 1;
                        let base = self.pos;
                        let raw = self.take_until(|b| b == q);
                        if self.peek().is_none() {
                            self.diag("eof-in-tag");
                            return None;
                        }
                        self.pos += 1;
                        value = self.decode_text(raw, base, RefCtx::Attr);
                    }
                    Some(b'>') => self.diag("missing-attribute-value"),
                    None => {
                        self.diag("eof-in-tag");
                        return None;
                    }
                    Some(_) => {
                        let base = self.pos;
                        let raw = self.take_until(|b| is_ws(b) || b == b'>');
                        value = self.decode_text(raw, base, RefCtx::Attr);
                    }
                }
            }
            if attrs.iter().any(|(n, _)| *n == aname) {
                self.diag("duplicate-attribute");
            } else {
                attrs.push((aname, value));
            }
        }
        if is_end {
            if !attrs.is_empty() {
                self.diag("end-tag-with-attributes");
            }
            if self_closing {
                self.diag("end-tag-with-trailing-solidus");
            }
            return Some(Token::EndTag { name });
        }
        if let Some(kind) = raw_kind_for(&name, self.scripting) {
            self.raw = Some((kind, name.clone()));
        }
        Some(Token::StartTag {
            name,
            attrs,
            self_closing,
        })
    }

    /// Finds the end of script data, honoring the escaped (`<!--`) and
    /// double-escaped (`<!-- <script`) states: `</script` only ends the element
    /// outside double-escaped text. One linear pass.
    fn find_script_end(&self) -> Option<usize> {
        fn script_tag(b: &[u8]) -> bool {
            b.len() > 6
                && b[..6].eq_ignore_ascii_case(b"script")
                && matches!(b[6], b' ' | b'\t' | b'\n' | 0x0c | b'/' | b'>')
        }
        let b = self.src.as_bytes();
        let mut i = self.pos;
        // 0 = script data, 1 = escaped, 2 = double escaped.
        let mut state = 0u8;
        while i < b.len() {
            let rest = &b[i..];
            match b[i] {
                b'-' if state != 0 && rest.starts_with(b"-->") => {
                    state = 0;
                    i += 3;
                }
                b'<' if state == 0 && rest.starts_with(b"<!--") => {
                    // Rescan from the first dash so `<!-->` and `<!--->` close at once.
                    state = 1;
                    i += 2;
                }
                b'<' if rest.starts_with(b"</") && script_tag(&rest[2..]) => {
                    if state == 2 {
                        state = 1;
                        i += 8;
                    } else {
                        return Some(i);
                    }
                }
                b'<' if state == 1 && script_tag(&rest[1..]) => {
                    state = 2;
                    i += 7;
                }
                _ => i += 1,
            }
        }
        None
    }

    /// Finds the byte index of the matching `</name` end tag in the remaining input.
    fn find_raw_end(&self, name: &str) -> Option<usize> {
        if name == "script" {
            return self.find_script_end();
        }
        let src = self.src;
        let b = src.as_bytes();
        let mut i = self.pos;
        while let Some(off) = src[i..].find("</") {
            let at = i + off;
            let after = at + 2 + name.len();
            if after <= b.len()
                && src[at + 2..after].eq_ignore_ascii_case(name)
                // EOF right after the name is not an end tag: the spec emits the
                // partial `</name` as text.
                && b.get(after)
                    .is_some_and(|&c| is_ws(c) || c == b'/' || c == b'>')
            {
                return Some(at);
            }
            i = at + 2;
        }
        None
    }

    fn lex_raw(&mut self, kind: RawKind, name: &str) -> Option<Token> {
        let src = self.src;
        if kind == RawKind::Plaintext {
            // PLAINTEXT never decodes character references.
            let text = &src[self.pos..];
            if text.contains('\0') {
                self.diag("unexpected-null-character");
            }
            self.pos = src.len();
            return Some(Token::Character(text.replace('\0', "\u{FFFD}")));
        }
        let end = self.find_raw_end(name);
        if end == Some(self.pos) {
            self.raw = None;
            self.pos += 2;
            return self.lex_tag(true);
        }
        let stop = end.unwrap_or(src.len());
        let base = self.pos;
        self.pos = stop;
        let text = &src[base..stop];
        let out = if kind == RawKind::RcData {
            self.decode_text(text, base, RefCtx::RcData)
        } else {
            if text.contains('\0') {
                self.diag("unexpected-null-character");
            }
            text.replace('\0', "\u{FFFD}")
        };
        Some(Token::Character(out))
    }
}

/// Tokenizes a whole input (newline-normalized, scripting enabled). Test helper.
pub fn tokenize(src: &str) -> Vec<Token> {
    let mut t = Tokenizer::new(src, true);
    let mut out = Vec::new();
    loop {
        let tok = t.next_token().token;
        if tok == Token::Eof {
            return out;
        }
        out.push(tok);
    }
}

/// ASCII-lowercases a tag or attribute name, replacing NUL with U+FFFD.
/// Performs a single allocation (none of the extra `replace` copy).
fn lower_name(raw: &str) -> String {
    let mut out = raw.to_ascii_lowercase();
    if out.contains('\0') {
        out = out.replace('\0', "\u{FFFD}");
    }
    out
}
