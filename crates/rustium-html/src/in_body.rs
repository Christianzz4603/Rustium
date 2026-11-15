//! The "in body" insertion mode. The table modes it hands off to are in
//! `tables.rs`.

use crate::token::Token;
use crate::tree_builder::{is_special, Fmt, Mode, Scope, TreeBuilder};

const VOID: &[&str] = &[
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
    "keygen", "link", "meta", "param", "source", "track", "wbr",
];
const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];

impl TreeBuilder {
    pub(crate) fn in_body(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Character(s) => {
                self.reconstruct_formatting();
                self.insert_text(&s);
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
            } => {
                self.body_start_tag(&name, attrs, self_closing);
                None
            }
            Token::EndTag { name } => self.body_end_tag(&name),
            Token::Eof => None,
        }
    }

    fn list_item(&mut self, names: &[&str]) {
        let mut i = self.open.len();
        while i > 0 {
            i -= 1;
            let node = self.open[i];
            let nm = self.name(node);
            if names.contains(&nm.as_str()) {
                self.generate_implied_end_tags(Some(nm.as_str()));
                if self.open.last() != Some(&node) {
                    self.error("unclosed-element-in-list-item");
                }
                self.open.truncate(i);
                break;
            }
            if is_special(nm.as_str()) && !matches!(nm.as_str(), "address" | "div" | "p") {
                break;
            }
        }
        self.close_p_if_in_button_scope();
    }

    fn enter_text_mode(&mut self) {
        self.orig_mode = self.mode;
        self.mode = Mode::Text;
    }

    fn body_start_tag(&mut self, n: &str, attrs: Vec<(String, String)>, self_closing: bool) {
        match n {
            "html" => {
                self.error("unexpected-html-start-tag");
                if let Some(&root) = self.open.first() {
                    for (k, v) in &attrs {
                        self.dom.set_attribute_if_absent(root, k, v);
                    }
                }
            }
            "base" | "basefont" | "bgsound" | "link" | "meta" | "noframes" | "script" | "style"
            | "template" | "title" => {
                self.in_head(Token::StartTag {
                    name: n.to_string(),
                    attrs,
                    self_closing,
                });
                return;
            }
            "body" => {
                self.error("unexpected-body-start-tag");
                if self.open.len() >= 2 && self.is(self.open[1], "body") {
                    let body = self.open[1];
                    for (k, v) in &attrs {
                        self.dom.set_attribute_if_absent(body, k, v);
                    }
                }
            }
            "frameset" | "frame" => self.unsupported("frameset-not-implemented"),
            "head" => self.error("unexpected-head-start-tag"),
            "address" | "article" | "aside" | "blockquote" | "center" | "details" | "dialog"
            | "dir" | "div" | "dl" | "fieldset" | "figcaption" | "figure" | "footer" | "header"
            | "hgroup" | "main" | "menu" | "nav" | "ol" | "p" | "search" | "section"
            | "summary" | "ul" => {
                self.close_p_if_in_button_scope();
                self.insert_element(n, &attrs);
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.close_p_if_in_button_scope();
                if let Some(&cur) = self.open.last() {
                    if HEADINGS.contains(&self.name(cur).as_str()) {
                        self.error("nested-heading");
                        self.pop();
                    }
                }
                self.insert_element(n, &attrs);
            }
            "pre" | "listing" => {
                self.close_p_if_in_button_scope();
                self.insert_element(n, &attrs);
                self.ignore_lf = true;
            }
            "form" => {
                if self.form.is_some() {
                    self.error("nested-form");
                    return;
                }
                self.close_p_if_in_button_scope();
                self.form = Some(self.insert_element(n, &attrs));
            }
            "li" => {
                self.list_item(&["li"]);
                self.insert_element(n, &attrs);
            }
            "dd" | "dt" => {
                self.list_item(&["dd", "dt"]);
                self.insert_element(n, &attrs);
            }
            "plaintext" => {
                self.close_p_if_in_button_scope();
                self.insert_element(n, &attrs);
            }
            "button" => {
                if self.has_in_scope("button", Scope::Default) {
                    self.error("nested-button");
                    self.generate_implied_end_tags(None);
                    self.pop_until("button");
                }
                self.reconstruct_formatting();
                self.insert_element(n, &attrs);
            }
            "a" => {
                if let Some(i) = self.find_formatting("a") {
                    self.error("nested-anchor");
                    let old = match &self.formatting[i] {
                        Fmt::Element { id, .. } => Some(*id),
                        Fmt::Marker => None,
                    };
                    self.adoption_agency("a");
                    if let Some(old) = old {
                        self.formatting
                            .retain(|f| !matches!(f, Fmt::Element { id, .. } if *id == old));
                        self.open.retain(|&x| x != old);
                    }
                }
                self.reconstruct_formatting();
                let id = self.insert_element(n, &attrs);
                self.push_formatting(id, n, &attrs);
            }
            "b" | "big" | "code" | "em" | "font" | "i" | "s" | "small" | "strike" | "strong"
            | "tt" | "u" => {
                self.reconstruct_formatting();
                let id = self.insert_element(n, &attrs);
                self.push_formatting(id, n, &attrs);
            }
            "nobr" => {
                self.reconstruct_formatting();
                if self.has_in_scope("nobr", Scope::Default) {
                    self.error("nested-nobr");
                    self.adoption_agency("nobr");
                    self.reconstruct_formatting();
                }
                let id = self.insert_element(n, &attrs);
                self.push_formatting(id, n, &attrs);
            }
            "applet" | "marquee" | "object" => {
                self.reconstruct_formatting();
                self.insert_element(n, &attrs);
                self.push_marker();
            }
            "table" => {
                if !self.quirks {
                    self.close_p_if_in_button_scope();
                }
                self.insert_element(n, &attrs);
                self.mode = Mode::InTable;
            }
            "caption" | "col" | "colgroup" | "tbody" | "td" | "tfoot" | "th" | "thead" | "tr" => {
                self.error("unexpected-table-part-start-tag");
            }
            "area" | "br" | "embed" | "img" | "keygen" | "wbr" | "input" => {
                self.reconstruct_formatting();
                self.insert_element(n, &attrs);
                self.pop();
            }
            "param" | "source" | "track" => {
                self.insert_element(n, &attrs);
                self.pop();
            }
            "hr" => {
                self.close_p_if_in_button_scope();
                self.insert_element(n, &attrs);
                self.pop();
            }
            "image" => {
                self.error("image-start-tag");
                self.body_start_tag("img", attrs, self_closing);
                return;
            }
            "textarea" => {
                self.insert_element(n, &attrs);
                self.ignore_lf = true;
                self.enter_text_mode();
            }
            "xmp" => {
                self.close_p_if_in_button_scope();
                self.reconstruct_formatting();
                self.insert_element(n, &attrs);
                self.enter_text_mode();
            }
            "iframe" | "noembed" => {
                self.insert_element(n, &attrs);
                self.enter_text_mode();
            }
            "noscript" if self.scripting => {
                self.insert_element(n, &attrs);
                self.enter_text_mode();
            }
            "select" => {
                self.reconstruct_formatting();
                self.insert_element(n, &attrs);
                self.unsupported("select-mode-not-implemented");
            }
            "optgroup" | "option" => {
                if self.current_is("option") {
                    self.pop();
                }
                self.reconstruct_formatting();
                self.insert_element(n, &attrs);
            }
            "rb" | "rtc" => {
                if self.has_in_scope("ruby", Scope::Default) {
                    self.generate_implied_end_tags(None);
                }
                self.insert_element(n, &attrs);
            }
            "rp" | "rt" => {
                if self.has_in_scope("ruby", Scope::Default) {
                    self.generate_implied_end_tags(Some("rtc"));
                }
                self.insert_element(n, &attrs);
            }
            "math" | "svg" => {
                self.reconstruct_formatting();
                self.insert_element(n, &attrs);
                self.unsupported("foreign-content-not-implemented");
                if self_closing {
                    self.pop();
                    return;
                }
            }
            _ => {
                self.reconstruct_formatting();
                self.insert_element(n, &attrs);
            }
        }
        if self_closing && !VOID.contains(&n) {
            self.error("non-void-html-element-start-tag-with-trailing-solidus");
        }
    }

    fn body_end_tag(&mut self, n: &str) -> Option<Token> {
        match n {
            "body" => {
                if !self.has_in_scope("body", Scope::Default) {
                    self.error("unexpected-body-end-tag");
                } else {
                    self.mode = Mode::AfterBody;
                }
            }
            "html" => {
                if !self.has_in_scope("body", Scope::Default) {
                    self.error("unexpected-html-end-tag");
                } else {
                    self.mode = Mode::AfterBody;
                    return Some(Token::EndTag {
                        name: n.to_string(),
                    });
                }
            }
            "address" | "article" | "aside" | "blockquote" | "button" | "center" | "details"
            | "dialog" | "dir" | "div" | "dl" | "fieldset" | "figcaption" | "figure" | "footer"
            | "header" | "hgroup" | "listing" | "main" | "menu" | "nav" | "ol" | "pre"
            | "search" | "section" | "select" | "summary" | "ul" => {
                if !self.has_in_scope(n, Scope::Default) {
                    self.error("unexpected-end-tag");
                } else {
                    self.generate_implied_end_tags(None);
                    if !self.current_is(n) {
                        self.error("unclosed-element-at-end-tag");
                    }
                    self.pop_until(n);
                }
            }
            "form" => match self.form.take() {
                Some(f) if self.node_in_scope(f) => {
                    self.generate_implied_end_tags(None);
                    if self.open.last() != Some(&f) {
                        self.error("unclosed-element-at-end-tag");
                    }
                    self.open.retain(|&x| x != f);
                }
                _ => self.error("unexpected-form-end-tag"),
            },
            "p" => {
                if !self.has_in_scope("p", Scope::Button) {
                    self.error("unexpected-p-end-tag");
                    self.insert_element("p", &[]);
                }
                self.close_p_if_in_button_scope();
            }
            "li" => {
                if !self.has_in_scope("li", Scope::ListItem) {
                    self.error("unexpected-end-tag");
                } else {
                    self.generate_implied_end_tags(Some("li"));
                    if !self.current_is("li") {
                        self.error("unclosed-element-at-end-tag");
                    }
                    self.pop_until("li");
                }
            }
            "dd" | "dt" => {
                if !self.has_in_scope(n, Scope::Default) {
                    self.error("unexpected-end-tag");
                } else {
                    self.generate_implied_end_tags(Some(n));
                    if !self.current_is(n) {
                        self.error("unclosed-element-at-end-tag");
                    }
                    self.pop_until(n);
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                if !self.has_in_scope_any(HEADINGS, Scope::Default) {
                    self.error("unexpected-end-tag");
                } else {
                    self.generate_implied_end_tags(None);
                    if !self.current_is(n) {
                        self.error("unclosed-element-at-end-tag");
                    }
                    self.pop_until_any(HEADINGS);
                }
            }
            "a" | "b" | "big" | "code" | "em" | "font" | "i" | "nobr" | "s" | "small"
            | "strike" | "strong" | "tt" | "u" => {
                if !self.adoption_agency(n) {
                    self.any_other_end_tag(n);
                }
            }
            "applet" | "marquee" | "object" => {
                if !self.has_in_scope(n, Scope::Default) {
                    self.error("unexpected-end-tag");
                } else {
                    self.generate_implied_end_tags(None);
                    if !self.current_is(n) {
                        self.error("unclosed-element-at-end-tag");
                    }
                    self.pop_until(n);
                    self.clear_formatting_to_marker();
                }
            }
            "br" => {
                self.error("end-tag-br");
                self.body_start_tag("br", Vec::new(), false);
            }
            "template" => {
                return self.in_head(Token::EndTag {
                    name: n.to_string(),
                })
            }
            _ => self.any_other_end_tag(n),
        }
        None
    }
}
