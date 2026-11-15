//! The frameset insertion modes: in frameset, after frameset, after after
//! frameset, plus the `frameset-ok` flag and the in-body `<frameset>` rule.

use crate::token::Token;
use crate::tree_builder::{split_leading_ws, Mode, TreeBuilder};

/// Start tags that set the `frameset-ok` flag to "not ok" when seen in body.
/// (`input` is handled separately: only non-hidden inputs count.)
fn clears_frameset_ok(name: &str, attrs: &[(String, String)]) -> bool {
    match name {
        "body" | "li" | "dd" | "dt" | "pre" | "listing" | "button" | "applet" | "marquee"
        | "object" | "table" | "area" | "br" | "embed" | "img" | "image" | "keygen" | "wbr"
        | "hr" | "textarea" | "xmp" | "iframe" | "select" => true,
        "input" => !attrs
            .iter()
            .any(|(k, v)| k == "type" && v.eq_ignore_ascii_case("hidden")),
        _ => false,
    }
}

impl TreeBuilder {
    /// Called for every in-body start tag before it is handled.
    pub(crate) fn note_frameset_ok(&mut self, name: &str, attrs: &[(String, String)]) {
        if clears_frameset_ok(name, attrs) {
            self.frameset_ok = false;
        }
    }

    /// In body: `<frameset>` start tag.
    pub(crate) fn body_frameset(&mut self, attrs: &[(String, String)]) {
        self.error("unexpected-frameset-start-tag");
        if self.open.len() < 2 || !self.is(self.open[1], "body") || !self.frameset_ok {
            return; // ignored
        }
        let body = self.open[1];
        self.dom.detach(body);
        self.open.truncate(1);
        self.insert_element("frameset", attrs);
        self.mode = Mode::InFrameset;
    }

    /// After head: `<frameset>` start tag.
    pub(crate) fn head_frameset(&mut self, attrs: &[(String, String)]) {
        self.insert_element("frameset", attrs);
        self.mode = Mode::InFrameset;
    }

    pub(crate) fn in_frameset(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Character(s) => {
                let ws: String = s
                    .chars()
                    .filter(|c| matches!(c, ' ' | '\t' | '\n' | '\x0c'))
                    .collect();
                if ws.len() != s.len() {
                    self.error("unexpected-char-in-frameset");
                }
                self.insert_text(&ws);
                None
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
                "frameset" => {
                    self.insert_element("frameset", &attrs);
                    None
                }
                "frame" => {
                    self.insert_element("frame", &attrs);
                    self.pop();
                    None
                }
                "noframes" => self.in_head(Token::StartTag {
                    name,
                    attrs,
                    self_closing,
                }),
                _ => {
                    self.error("unexpected-start-tag-in-frameset");
                    None
                }
            },
            Token::EndTag { name } if name == "frameset" => {
                if self.open.len() <= 1 {
                    self.error("unexpected-frameset-end-tag");
                    return None;
                }
                self.pop();
                if self.context.is_none() && !self.is(self.current(), "frameset") {
                    self.mode = Mode::AfterFrameset;
                }
                None
            }
            Token::EndTag { .. } => {
                self.error("unexpected-end-tag-in-frameset");
                None
            }
            Token::Eof => {
                if self.open.len() > 1 {
                    self.error("eof-in-frameset");
                }
                None
            }
        }
    }

    pub(crate) fn after_frameset(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Character(s) => {
                let ws: String = s
                    .chars()
                    .filter(|c| matches!(c, ' ' | '\t' | '\n' | '\x0c'))
                    .collect();
                if ws.len() != s.len() {
                    self.error("unexpected-char-after-frameset");
                }
                self.insert_text(&ws);
                None
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
                "noframes" => self.in_head(Token::StartTag {
                    name,
                    attrs,
                    self_closing,
                }),
                _ => {
                    self.error("unexpected-start-tag-after-frameset");
                    None
                }
            },
            Token::EndTag { name } if name == "html" => {
                self.mode = Mode::AfterAfterFrameset;
                None
            }
            Token::EndTag { .. } => {
                self.error("unexpected-end-tag-after-frameset");
                None
            }
            Token::Eof => None,
        }
    }

    pub(crate) fn after_after_frameset(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Comment(c) => {
                let doc = self.dom.document();
                self.insert_comment(&c, Some(doc));
                None
            }
            Token::Doctype { .. } => None,
            Token::Character(s) => {
                let rest = split_leading_ws(&s).1;
                let all: String = s
                    .chars()
                    .filter(|c| matches!(c, ' ' | '\t' | '\n' | '\x0c'))
                    .collect();
                if !all.is_empty() {
                    self.in_body(Token::Character(all));
                }
                if !rest.is_empty() {
                    self.error("unexpected-char-after-after-frameset");
                }
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
                "noframes" => self.in_head(Token::StartTag {
                    name,
                    attrs,
                    self_closing,
                }),
                _ => {
                    self.error("unexpected-start-tag-after-after-frameset");
                    None
                }
            },
            Token::EndTag { .. } => {
                self.error("unexpected-end-tag-after-after-frameset");
                None
            }
            Token::Eof => None,
        }
    }
}
