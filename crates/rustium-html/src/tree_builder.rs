//! Tree construction: shared state, helpers, and the pre-body insertion modes.
//!
//! Implemented insertion modes: initial, before html, before head, in head,
//! after head, in body, text, after body, after after body.
//!
//! NOT implemented (reported as `Unsupported` diagnostics where encountered):
//! in table / in select / in frameset / in template modes (tables use a
//! simplified rule set with no foster parenting and no implied `tbody`),
//! foreign content (svg/math are plain elements), and quirks mode.

use rustium_common::{Atom, Diagnostic, Severity, Span};
use rustium_dom::{Dom, NodeData, NodeId};

use crate::token::Token;

/// Maximum depth of the stack of open elements. Elements opened beyond this
/// are attached to the current node but never become the insertion point,
/// bounding recursion in later stages (same strategy as other engines).
pub const MAX_OPEN_DEPTH: usize = 512;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Mode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    AfterHead,
    InBody,
    Text,
    AfterBody,
    AfterAfterBody,
}

#[derive(Clone, Debug)]
pub(crate) enum Fmt {
    Marker,
    Element {
        id: NodeId,
        name: String,
        attrs: Vec<(String, String)>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    Default,
    ListItem,
    Button,
    Table,
}

const SCOPE_BASE: &[&str] = &[
    "applet", "caption", "html", "table", "td", "th", "marquee", "object", "template",
];

pub(crate) fn is_special(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "applet"
            | "area"
            | "article"
            | "aside"
            | "base"
            | "basefont"
            | "bgsound"
            | "blockquote"
            | "body"
            | "br"
            | "button"
            | "caption"
            | "center"
            | "col"
            | "colgroup"
            | "dd"
            | "details"
            | "dir"
            | "div"
            | "dl"
            | "dt"
            | "embed"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "frame"
            | "frameset"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hgroup"
            | "hr"
            | "html"
            | "iframe"
            | "img"
            | "input"
            | "keygen"
            | "li"
            | "link"
            | "listing"
            | "main"
            | "marquee"
            | "menu"
            | "meta"
            | "nav"
            | "noembed"
            | "noframes"
            | "noscript"
            | "object"
            | "ol"
            | "p"
            | "param"
            | "plaintext"
            | "pre"
            | "script"
            | "search"
            | "section"
            | "select"
            | "source"
            | "style"
            | "summary"
            | "table"
            | "tbody"
            | "td"
            | "template"
            | "textarea"
            | "tfoot"
            | "th"
            | "thead"
            | "title"
            | "tr"
            | "track"
            | "ul"
            | "wbr"
            | "xmp"
    )
}

pub(crate) fn split_leading_ws(s: &str) -> (&str, &str) {
    let n = s
        .bytes()
        .take_while(|b| matches!(b, b' ' | b'\t' | b'\n' | 0x0c))
        .count();
    s.split_at(n)
}

pub(crate) struct TreeBuilder {
    pub dom: Dom,
    pub mode: Mode,
    pub orig_mode: Mode,
    pub open: Vec<NodeId>,
    pub formatting: Vec<Fmt>,
    pub head: Option<NodeId>,
    pub form: Option<NodeId>,
    pub ignore_lf: bool,
    pub diags: Vec<Diagnostic>,
    pub span: Span,
    pub scripting: bool,
    pub after_head_pending_head: Option<NodeId>,
    depth_reported: bool,
}

impl TreeBuilder {
    pub fn new(dom: Dom, scripting: bool) -> Self {
        TreeBuilder {
            dom,
            mode: Mode::Initial,
            orig_mode: Mode::Initial,
            open: Vec::new(),
            formatting: Vec::new(),
            head: None,
            form: None,
            ignore_lf: false,
            diags: Vec::new(),
            span: Span::default(),
            scripting,
            after_head_pending_head: None,
            depth_reported: false,
        }
    }

    // ----- diagnostics -----

    pub fn error(&mut self, code: &'static str) {
        self.diags.push(Diagnostic::parse_error(code, self.span));
    }

    pub fn unsupported(&mut self, code: &'static str) {
        self.diags.push(Diagnostic {
            severity: Severity::Unsupported,
            code,
            span: self.span,
        });
    }

    // ----- node helpers -----

    pub fn is(&self, id: NodeId, name: &str) -> bool {
        matches!(&self.dom.node(id).data, NodeData::Element(e) if e.name == name)
    }

    pub fn name(&self, id: NodeId) -> Atom {
        match &self.dom.node(id).data {
            NodeData::Element(e) => e.name.clone(),
            _ => Atom::new(""),
        }
    }

    pub fn current(&self) -> NodeId {
        *self.open.last().unwrap_or(&self.dom.document())
    }

    pub fn current_is(&self, name: &str) -> bool {
        self.open.last().is_some_and(|&n| self.is(n, name))
    }

    pub fn create_element(&mut self, name: &str, attrs: &[(String, String)]) -> NodeId {
        let id = self.dom.create_element(name);
        for (k, v) in attrs {
            self.dom.set_attribute(id, k, v);
        }
        id
    }

    /// Creates an element, appends it at the insertion point, and pushes it on the stack.
    pub fn insert_element(&mut self, name: &str, attrs: &[(String, String)]) -> NodeId {
        let id = self.create_element(name, attrs);
        let parent = self.current();
        let _ = self.dom.append_child(parent, id);
        if self.open.len() < MAX_OPEN_DEPTH {
            self.open.push(id);
        } else if !self.depth_reported {
            self.depth_reported = true;
            self.unsupported("max-open-element-depth-exceeded");
        }
        id
    }

    pub fn insert_text(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        let parent = self.current();
        if parent == self.dom.document() {
            return; // text is never a child of Document
        }
        self.dom.append_text(parent, s);
    }

    pub fn insert_comment(&mut self, s: &str, parent: Option<NodeId>) {
        let parent = parent.unwrap_or_else(|| self.current());
        let c = self.dom.create_comment(s);
        let _ = self.dom.append_child(parent, c);
    }

    pub fn pop(&mut self) {
        self.open.pop();
    }

    // ----- scope and stack helpers -----

    pub fn has_in_scope(&self, target: &str, scope: Scope) -> bool {
        self.has_in_scope_any(&[target], scope)
    }

    pub fn has_in_scope_any(&self, targets: &[&str], scope: Scope) -> bool {
        for &n in self.open.iter().rev() {
            let name = self.name(n);
            if targets.contains(&name.as_str()) {
                return true;
            }
            let stop = match scope {
                Scope::Table => matches!(name.as_str(), "html" | "table" | "template"),
                Scope::Default => SCOPE_BASE.contains(&name.as_str()),
                Scope::ListItem => {
                    SCOPE_BASE.contains(&name.as_str()) || matches!(name.as_str(), "ol" | "ul")
                }
                Scope::Button => SCOPE_BASE.contains(&name.as_str()) || name == "button",
            };
            if stop {
                return false;
            }
        }
        false
    }

    pub fn node_in_scope(&self, id: NodeId) -> bool {
        for &n in self.open.iter().rev() {
            if n == id {
                return true;
            }
            if SCOPE_BASE.contains(&self.name(n).as_str()) {
                return false;
            }
        }
        false
    }

    pub fn generate_implied_end_tags(&mut self, except: Option<&str>) {
        while let Some(&n) = self.open.last() {
            let name = self.name(n);
            let implied = matches!(
                name.as_str(),
                "dd" | "dt" | "li" | "optgroup" | "option" | "p" | "rb" | "rp" | "rt" | "rtc"
            );
            if implied && Some(name.as_str()) != except {
                self.open.pop();
            } else {
                break;
            }
        }
    }

    /// Pops until an element named `name` has been popped.
    pub fn pop_until(&mut self, name: &str) {
        while let Some(n) = self.open.pop() {
            if self.is(n, name) {
                break;
            }
        }
    }

    pub fn pop_until_any(&mut self, names: &[&str]) {
        while let Some(n) = self.open.pop() {
            if names.contains(&self.name(n).as_str()) {
                break;
            }
        }
    }

    pub fn close_p_if_in_button_scope(&mut self) {
        if self.has_in_scope("p", Scope::Button) {
            self.generate_implied_end_tags(Some("p"));
            if !self.current_is("p") {
                self.error("unclosed-element-before-p-end");
            }
            self.pop_until("p");
        }
    }

    // ----- driver -----

    pub fn process(&mut self, mut tok: Token) {
        if self.ignore_lf {
            self.ignore_lf = false;
            if let Token::Character(s) = &mut tok {
                if s.starts_with('\n') {
                    s.remove(0);
                    if s.is_empty() {
                        return;
                    }
                }
            }
        }
        loop {
            let next = match self.mode {
                Mode::Initial => self.initial(tok),
                Mode::BeforeHtml => self.before_html(tok),
                Mode::BeforeHead => self.before_head(tok),
                Mode::InHead => self.in_head(tok),
                Mode::AfterHead => self.after_head(tok),
                Mode::InBody => self.in_body(tok),
                Mode::Text => self.text_mode(tok),
                Mode::AfterBody => self.after_body(tok),
                Mode::AfterAfterBody => self.after_after_body(tok),
            };
            match next {
                Some(t) => tok = t,
                None => return,
            }
        }
    }

    // ----- pre-body modes (each returns Some(token) to reprocess) -----

    fn initial(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Character(s) => {
                let (_, rest) = split_leading_ws(&s);
                if rest.is_empty() {
                    return None;
                }
                let rest = rest.to_string();
                self.error("missing-doctype");
                self.mode = Mode::BeforeHtml;
                Some(Token::Character(rest))
            }
            Token::Comment(c) => {
                let doc = self.dom.document();
                self.insert_comment(&c, Some(doc));
                None
            }
            Token::Doctype { name } => {
                let d = self.dom.create_doctype(name.as_deref().unwrap_or(""));
                let doc = self.dom.document();
                let _ = self.dom.append_child(doc, d);
                self.unsupported("quirks-mode-not-implemented");
                self.mode = Mode::BeforeHtml;
                None
            }
            other => {
                self.error("missing-doctype");
                self.mode = Mode::BeforeHtml;
                Some(other)
            }
        }
    }

    fn before_html(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Doctype { .. } => {
                self.error("unexpected-doctype");
                None
            }
            Token::Comment(c) => {
                let doc = self.dom.document();
                self.insert_comment(&c, Some(doc));
                None
            }
            Token::Character(s) => {
                let (_, rest) = split_leading_ws(&s);
                if rest.is_empty() {
                    return None;
                }
                let rest = rest.to_string();
                self.create_root_html(&[]);
                self.mode = Mode::BeforeHead;
                Some(Token::Character(rest))
            }
            Token::StartTag { name, attrs, .. } if name == "html" => {
                self.create_root_html(&attrs);
                self.mode = Mode::BeforeHead;
                None
            }
            Token::EndTag { name } if !matches!(name.as_str(), "head" | "body" | "html" | "br") => {
                self.error("unexpected-end-tag");
                None
            }
            other => {
                self.create_root_html(&[]);
                self.mode = Mode::BeforeHead;
                Some(other)
            }
        }
    }

    pub(crate) fn create_root_html(&mut self, attrs: &[(String, String)]) {
        let html = self.create_element("html", attrs);
        let doc = self.dom.document();
        let _ = self.dom.append_child(doc, html);
        self.open.push(html);
    }

    fn before_head(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Character(s) => {
                let (_, rest) = split_leading_ws(&s);
                if rest.is_empty() {
                    return None;
                }
                let rest = rest.to_string();
                self.insert_implied_head();
                Some(Token::Character(rest))
            }
            Token::Comment(c) => {
                self.insert_comment(&c, None);
                None
            }
            Token::Doctype { .. } => {
                self.error("unexpected-doctype");
                None
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if name == "html" => self.in_body(Token::StartTag {
                name,
                attrs,
                self_closing,
            }),
            Token::StartTag { name, attrs, .. } if name == "head" => {
                self.head = Some(self.insert_element("head", &attrs));
                self.mode = Mode::InHead;
                None
            }
            Token::EndTag { name } if !matches!(name.as_str(), "head" | "body" | "html" | "br") => {
                self.error("unexpected-end-tag");
                None
            }
            other => {
                self.insert_implied_head();
                Some(other)
            }
        }
    }

    fn insert_implied_head(&mut self) {
        self.head = Some(self.insert_element("head", &[]));
        self.mode = Mode::InHead;
    }

    pub(crate) fn in_head(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Character(s) => {
                let (ws, rest) = split_leading_ws(&s);
                self.insert_text(ws);
                if rest.is_empty() {
                    return None;
                }
                let rest = rest.to_string();
                self.pop();
                self.mode = Mode::AfterHead;
                Some(Token::Character(rest))
            }
            Token::Comment(c) => {
                self.insert_comment(&c, None);
                None
            }
            Token::Doctype { .. } => {
                self.error("unexpected-doctype");
                None
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => match name.as_str() {
                "html" => self.in_body(Token::StartTag {
                    name,
                    attrs,
                    self_closing,
                }),
                "base" | "basefont" | "bgsound" | "link" | "meta" => {
                    self.insert_element(&name, &attrs);
                    self.pop();
                    None
                }
                "title" | "style" | "script" | "noframes" => {
                    self.insert_element(&name, &attrs);
                    self.orig_mode = self.mode;
                    self.mode = Mode::Text;
                    None
                }
                "noscript" if self.scripting => {
                    self.insert_element(&name, &attrs);
                    self.orig_mode = self.mode;
                    self.mode = Mode::Text;
                    None
                }
                "noscript" => {
                    self.unsupported("noscript-in-head-without-scripting");
                    self.insert_element(&name, &attrs);
                    None
                }
                "template" => {
                    self.unsupported("template-contents-not-implemented");
                    self.insert_element(&name, &attrs);
                    None
                }
                "head" => {
                    self.error("unexpected-head-start-tag");
                    None
                }
                _ => {
                    self.pop();
                    self.mode = Mode::AfterHead;
                    Some(Token::StartTag {
                        name,
                        attrs,
                        self_closing,
                    })
                }
            },
            Token::EndTag { name } => match name.as_str() {
                "head" => {
                    self.pop();
                    self.mode = Mode::AfterHead;
                    None
                }
                "template" => {
                    if self.has_in_scope("template", Scope::Default) {
                        self.generate_implied_end_tags(None);
                        self.pop_until("template");
                    } else {
                        self.error("unexpected-end-tag");
                    }
                    None
                }
                "body" | "html" | "br" => {
                    self.pop();
                    self.mode = Mode::AfterHead;
                    Some(Token::EndTag { name })
                }
                _ => {
                    self.error("unexpected-end-tag");
                    None
                }
            },
            Token::Eof => {
                self.pop();
                self.mode = Mode::AfterHead;
                Some(Token::Eof)
            }
        }
    }

    fn after_head(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Character(s) => {
                let (ws, rest) = split_leading_ws(&s);
                self.insert_text(ws);
                if rest.is_empty() {
                    return None;
                }
                let rest = rest.to_string();
                self.insert_implied_body();
                Some(Token::Character(rest))
            }
            Token::Comment(c) => {
                self.insert_comment(&c, None);
                None
            }
            Token::Doctype { .. } => {
                self.error("unexpected-doctype");
                None
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => match name.as_str() {
                "html" => self.in_body(Token::StartTag {
                    name,
                    attrs,
                    self_closing,
                }),
                "body" => {
                    self.insert_element("body", &attrs);
                    self.mode = Mode::InBody;
                    None
                }
                "frameset" => {
                    self.unsupported("frameset-not-implemented");
                    None
                }
                "base" | "basefont" | "bgsound" | "link" | "meta" | "noframes" | "script"
                | "style" | "template" | "title" => {
                    self.error("head-element-after-head");
                    if let Some(h) = self.head {
                        self.open.push(h);
                        let r = self.in_head(Token::StartTag {
                            name,
                            attrs,
                            self_closing,
                        });
                        debug_assert!(r.is_none());
                        // For raw-text elements the head must stay open until the
                        // text mode ends; remove it from the stack only afterwards.
                        if self.mode != Mode::Text {
                            self.open.retain(|&n| n != h);
                        } else {
                            self.after_head_pending_head = Some(h);
                        }
                    }
                    None
                }
                "head" => {
                    self.error("unexpected-head-start-tag");
                    None
                }
                _ => {
                    self.insert_implied_body();
                    Some(Token::StartTag {
                        name,
                        attrs,
                        self_closing,
                    })
                }
            },
            Token::EndTag { name } => match name.as_str() {
                "template" => self.in_head(Token::EndTag { name }),
                "body" | "html" | "br" => {
                    self.insert_implied_body();
                    Some(Token::EndTag { name })
                }
                _ => {
                    self.error("unexpected-end-tag");
                    None
                }
            },
            Token::Eof => {
                self.insert_implied_body();
                Some(Token::Eof)
            }
        }
    }

    fn insert_implied_body(&mut self) {
        self.insert_element("body", &[]);
        self.mode = Mode::InBody;
    }

    pub(crate) fn text_mode(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Character(s) => {
                self.insert_text(&s);
                None
            }
            Token::Eof => {
                self.error("eof-in-text-mode");
                self.pop();
                self.finish_text_mode();
                Some(Token::Eof)
            }
            Token::EndTag { .. } => {
                self.pop();
                self.finish_text_mode();
                None
            }
            _ => None,
        }
    }

    fn finish_text_mode(&mut self) {
        self.mode = self.orig_mode;
        if let Some(h) = self.after_head_pending_head.take() {
            self.open.retain(|&n| n != h);
        }
    }

    fn after_body(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Character(s) => {
                let (ws, rest) = split_leading_ws(&s);
                if !ws.is_empty() {
                    let ws = ws.to_string();
                    self.in_body(Token::Character(ws));
                }
                if rest.is_empty() {
                    return None;
                }
                self.error("unexpected-content-after-body");
                self.mode = Mode::InBody;
                Some(Token::Character(rest.to_string()))
            }
            Token::Comment(c) => {
                let html = self.open.first().copied();
                self.insert_comment(&c, html);
                None
            }
            Token::Doctype { .. } => {
                self.error("unexpected-doctype");
                None
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if name == "html" => self.in_body(Token::StartTag {
                name,
                attrs,
                self_closing,
            }),
            Token::EndTag { name } if name == "html" => {
                self.mode = Mode::AfterAfterBody;
                None
            }
            Token::Eof => None,
            other => {
                self.error("unexpected-content-after-body");
                self.mode = Mode::InBody;
                Some(other)
            }
        }
    }

    fn after_after_body(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Comment(c) => {
                let doc = self.dom.document();
                self.insert_comment(&c, Some(doc));
                None
            }
            Token::Doctype { .. } => None,
            Token::Character(s) => {
                let (ws, rest) = split_leading_ws(&s);
                if !ws.is_empty() {
                    let ws = ws.to_string();
                    self.in_body(Token::Character(ws));
                }
                if rest.is_empty() {
                    return None;
                }
                self.error("unexpected-content-after-body");
                self.mode = Mode::InBody;
                Some(Token::Character(rest.to_string()))
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if name == "html" => self.in_body(Token::StartTag {
                name,
                attrs,
                self_closing,
            }),
            Token::Eof => None,
            other => {
                self.error("unexpected-content-after-body");
                self.mode = Mode::InBody;
                Some(other)
            }
        }
    }
}
