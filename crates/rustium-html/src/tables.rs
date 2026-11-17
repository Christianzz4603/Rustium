//! The table insertion-mode family: in table, in table text, in caption,
//! in column group, in table body, in row, in cell. Also foster parenting
//! (see `TreeBuilder::appropriate_place`) and "reset the insertion mode".
//!
//! Not implemented: `<template>` interactions (template contexts, template
//! contents). The select modes live in `select.rs`.

use crate::token::Token;
use crate::tree_builder::{split_leading_ws, Mode, Scope, TreeBuilder};

const TABLE_CONTEXT: &[&str] = &["table"];
const BODY_CONTEXT: &[&str] = &["tbody", "tfoot", "thead"];
const ROW_CONTEXT: &[&str] = &["tr"];

impl TreeBuilder {
    /// Pops until the current node is one of `names`, `template` or `html`.
    fn clear_back_to(&mut self, names: &[&str]) {
        while let Some(&n) = self.open.last() {
            let nm = self.name(n);
            if names.contains(&nm.as_str()) || matches!(nm.as_str(), "template" | "html") {
                break;
            }
            self.open.pop();
        }
    }

    /// "Reset the insertion mode appropriately" (template case omitted).
    pub(crate) fn reset_insertion_mode(&mut self) {
        for i in (0..self.open.len()).rev() {
            let last = i == 0;
            let name = match (&self.context, last) {
                (Some(ctx), true) => ctx.clone(),
                _ => self.name(self.open[i]).to_string(),
            };
            let mode = match name.as_str() {
                "select" => {
                    // Inside a table (not stopped by a template): in select in table.
                    let mut m = Mode::InSelect;
                    if !last {
                        for j in (0..i).rev() {
                            match self.name(self.open[j]).as_str() {
                                "template" => break,
                                "table" => {
                                    m = Mode::InSelectInTable;
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    m
                }
                "td" | "th" if !last => Mode::InCell,
                "tr" => Mode::InRow,
                "tbody" | "thead" | "tfoot" => Mode::InTableBody,
                "caption" => Mode::InCaption,
                "colgroup" => Mode::InColumnGroup,
                "table" => Mode::InTable,
                "head" if !last => Mode::InHead,
                "body" => Mode::InBody,
                "frameset" => Mode::InFrameset,
                "html" => {
                    if self.head.is_none() {
                        Mode::BeforeHead
                    } else {
                        Mode::AfterHead
                    }
                }
                _ if last => Mode::InBody,
                _ => continue,
            };
            self.mode = mode;
            return;
        }
        self.mode = Mode::InBody;
    }

    /// The "anything else" rule of in table: process with in-body rules while
    /// foster parenting is enabled.
    fn table_anything_else(&mut self, tok: Token) -> Option<Token> {
        self.error("unexpected-token-in-table");
        self.foster_parenting = true;
        let r = self.in_body(tok);
        self.foster_parenting = false;
        r
    }

    pub(crate) fn in_table(&mut self, tok: Token) -> Option<Token> {
        match &tok {
            Token::Character(_) => {
                let cur = self.open.last().map(|&n| self.name(n));
                if cur.is_some_and(|c| {
                    matches!(c.as_str(), "table" | "tbody" | "tfoot" | "thead" | "tr")
                }) {
                    self.pending_table_text.clear();
                    self.orig_mode = self.mode;
                    self.mode = Mode::InTableText;
                    Some(tok)
                } else {
                    self.table_anything_else(tok)
                }
            }
            Token::Comment(c) => {
                self.insert_comment(c, None);
                None
            }
            Token::Doctype { .. } => {
                self.error("unexpected-doctype");
                None
            }
            Token::StartTag { name, attrs, .. } => match name.as_str() {
                "caption" => {
                    self.clear_back_to(TABLE_CONTEXT);
                    self.push_marker();
                    self.insert_element(name, attrs);
                    self.mode = Mode::InCaption;
                    None
                }
                "colgroup" => {
                    self.clear_back_to(TABLE_CONTEXT);
                    self.insert_element(name, attrs);
                    self.mode = Mode::InColumnGroup;
                    None
                }
                "col" => {
                    self.clear_back_to(TABLE_CONTEXT);
                    self.insert_element("colgroup", &[]);
                    self.mode = Mode::InColumnGroup;
                    Some(tok)
                }
                "tbody" | "tfoot" | "thead" => {
                    self.clear_back_to(TABLE_CONTEXT);
                    self.insert_element(name, attrs);
                    self.mode = Mode::InTableBody;
                    None
                }
                "td" | "th" | "tr" => {
                    self.clear_back_to(TABLE_CONTEXT);
                    self.insert_element("tbody", &[]);
                    self.mode = Mode::InTableBody;
                    Some(tok)
                }
                "table" => {
                    self.error("nested-table");
                    if !self.has_in_scope("table", Scope::Table) {
                        return None;
                    }
                    self.pop_until("table");
                    self.reset_insertion_mode();
                    Some(tok)
                }
                "style" | "script" | "template" => self.in_head(tok),
                "input" => {
                    let hidden = attrs
                        .iter()
                        .any(|(k, v)| k == "type" && v.eq_ignore_ascii_case("hidden"));
                    if hidden {
                        self.error("unexpected-hidden-input-in-table");
                        self.insert_element(name, attrs);
                        self.pop();
                        None
                    } else {
                        self.table_anything_else(tok)
                    }
                }
                "form" => {
                    self.error("unexpected-form-in-table");
                    if self.form.is_none() {
                        self.form = Some(self.insert_element(name, attrs));
                        self.pop();
                    }
                    None
                }
                _ => self.table_anything_else(tok),
            },
            Token::EndTag { name } => match name.as_str() {
                "table" => {
                    if !self.has_in_scope("table", Scope::Table) {
                        self.error("unexpected-end-tag");
                    } else {
                        self.pop_until("table");
                        self.reset_insertion_mode();
                    }
                    None
                }
                "body" | "caption" | "col" | "colgroup" | "html" | "tbody" | "td" | "tfoot"
                | "th" | "thead" | "tr" => {
                    self.error("unexpected-end-tag");
                    None
                }
                "template" => self.in_head(tok),
                _ => self.table_anything_else(tok),
            },
            Token::Eof => self.in_body(tok),
        }
    }

    pub(crate) fn in_table_text(&mut self, tok: Token) -> Option<Token> {
        if let Token::Character(s) = &tok {
            self.pending_table_text.push_str(s);
            return None;
        }
        let text = std::mem::take(&mut self.pending_table_text);
        let all_ws = text
            .bytes()
            .all(|b| matches!(b, b' ' | b'\t' | b'\n' | 0x0c));
        if all_ws {
            self.insert_text(&text);
        } else {
            self.error("non-space-in-table-text");
            self.foster_parenting = true;
            self.reconstruct_formatting();
            self.insert_text(&text);
            self.foster_parenting = false;
        }
        self.mode = self.orig_mode;
        Some(tok)
    }

    /// Closes the open `<caption>`; false if none is in table scope.
    fn close_caption(&mut self) -> bool {
        if !self.has_in_scope("caption", Scope::Table) {
            self.error("unexpected-caption-end");
            return false;
        }
        self.generate_implied_end_tags(None);
        if !self.current_is("caption") {
            self.error("unclosed-element-at-caption-end");
        }
        self.pop_until("caption");
        self.clear_formatting_to_marker();
        self.mode = Mode::InTable;
        true
    }

    pub(crate) fn in_caption(&mut self, tok: Token) -> Option<Token> {
        match &tok {
            Token::EndTag { name } if name == "caption" => {
                self.close_caption();
                None
            }
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "caption"
                        | "col"
                        | "colgroup"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) =>
            {
                self.close_caption().then_some(tok)
            }
            Token::EndTag { name } if name == "table" => self.close_caption().then_some(tok),
            Token::EndTag { name }
                if matches!(
                    name.as_str(),
                    "body"
                        | "col"
                        | "colgroup"
                        | "html"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) =>
            {
                self.error("unexpected-end-tag");
                None
            }
            _ => self.in_body(tok),
        }
    }

    pub(crate) fn in_column_group(&mut self, tok: Token) -> Option<Token> {
        match &tok {
            Token::Character(s) => {
                let (ws, rest) = split_leading_ws(s);
                self.insert_text(ws);
                if rest.is_empty() {
                    return None;
                }
                let rest = Token::Character(rest.to_string());
                self.leave_column_group(rest)
            }
            Token::Comment(c) => {
                self.insert_comment(c, None);
                None
            }
            Token::Doctype { .. } => {
                self.error("unexpected-doctype");
                None
            }
            Token::StartTag { name, attrs, .. } => match name.as_str() {
                "html" => self.in_body(tok),
                "col" => {
                    self.insert_element(name, attrs);
                    self.pop();
                    None
                }
                "template" => self.in_head(tok),
                _ => self.leave_column_group(tok),
            },
            Token::EndTag { name } => match name.as_str() {
                "colgroup" => {
                    if self.current_is("colgroup") {
                        self.pop();
                        self.mode = Mode::InTable;
                    } else {
                        self.error("unexpected-end-tag");
                    }
                    None
                }
                "col" => {
                    self.error("unexpected-end-tag");
                    None
                }
                "template" => self.in_head(tok),
                _ => self.leave_column_group(tok),
            },
            Token::Eof => self.in_body(tok),
        }
    }

    /// "Anything else" of in column group.
    fn leave_column_group(&mut self, tok: Token) -> Option<Token> {
        if !self.current_is("colgroup") {
            self.error("unexpected-token-in-column-group");
            return None;
        }
        self.pop();
        self.mode = Mode::InTable;
        Some(tok)
    }

    pub(crate) fn in_table_body(&mut self, tok: Token) -> Option<Token> {
        match &tok {
            Token::StartTag { name, attrs, .. } => match name.as_str() {
                "tr" => {
                    self.clear_back_to(BODY_CONTEXT);
                    self.insert_element(name, attrs);
                    self.mode = Mode::InRow;
                    None
                }
                "th" | "td" => {
                    self.error("cell-without-row");
                    self.clear_back_to(BODY_CONTEXT);
                    self.insert_element("tr", &[]);
                    self.mode = Mode::InRow;
                    Some(tok)
                }
                "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead" => {
                    self.leave_table_body(tok)
                }
                _ => self.in_table(tok),
            },
            Token::EndTag { name } => match name.as_str() {
                "tbody" | "tfoot" | "thead" => {
                    if !self.has_in_scope(name, Scope::Table) {
                        self.error("unexpected-end-tag");
                    } else {
                        self.clear_back_to(BODY_CONTEXT);
                        self.pop();
                        self.mode = Mode::InTable;
                    }
                    None
                }
                "table" => self.leave_table_body(tok),
                "body" | "caption" | "col" | "colgroup" | "html" | "td" | "th" | "tr" => {
                    self.error("unexpected-end-tag");
                    None
                }
                _ => self.in_table(tok),
            },
            _ => self.in_table(tok),
        }
    }

    fn leave_table_body(&mut self, tok: Token) -> Option<Token> {
        if !self.has_in_scope_any(BODY_CONTEXT, Scope::Table) {
            self.error("unexpected-token-in-table-body");
            return None;
        }
        self.clear_back_to(BODY_CONTEXT);
        self.pop();
        self.mode = Mode::InTable;
        Some(tok)
    }

    pub(crate) fn in_row(&mut self, tok: Token) -> Option<Token> {
        match &tok {
            Token::StartTag { name, attrs, .. } => match name.as_str() {
                "th" | "td" => {
                    self.clear_back_to(ROW_CONTEXT);
                    self.insert_element(name, attrs);
                    self.mode = Mode::InCell;
                    self.push_marker();
                    None
                }
                "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead" | "tr" => {
                    self.leave_row(tok)
                }
                _ => self.in_table(tok),
            },
            Token::EndTag { name } => match name.as_str() {
                "tr" => {
                    self.leave_row(tok);
                    None
                }
                "table" => self.leave_row(tok),
                "tbody" | "tfoot" | "thead" => {
                    if !self.has_in_scope(name, Scope::Table) {
                        self.error("unexpected-end-tag");
                        return None;
                    }
                    self.leave_row(tok)
                }
                "body" | "caption" | "col" | "colgroup" | "html" | "td" | "th" => {
                    self.error("unexpected-end-tag");
                    None
                }
                _ => self.in_table(tok),
            },
            _ => self.in_table(tok),
        }
    }

    /// Closes the current `<tr>` and returns `tok` for reprocessing, or
    /// `None` if no `<tr>` is in table scope. (For an end tag `tr` the caller
    /// discards the returned token, as the spec does not reprocess it.)
    fn leave_row(&mut self, tok: Token) -> Option<Token> {
        if !self.has_in_scope("tr", Scope::Table) {
            self.error("unexpected-token-in-row");
            return None;
        }
        self.clear_back_to(ROW_CONTEXT);
        self.pop();
        self.mode = Mode::InTableBody;
        Some(tok)
    }

    fn close_cell(&mut self) {
        self.generate_implied_end_tags(None);
        if !self
            .open
            .last()
            .is_some_and(|&n| matches!(self.name(n).as_str(), "td" | "th"))
        {
            self.error("unclosed-element-at-cell-end");
        }
        self.pop_until_any(&["td", "th"]);
        self.clear_formatting_to_marker();
        self.mode = Mode::InRow;
    }

    pub(crate) fn in_cell(&mut self, tok: Token) -> Option<Token> {
        match &tok {
            Token::EndTag { name } => match name.as_str() {
                "td" | "th" => {
                    if !self.has_in_scope(name, Scope::Table) {
                        self.error("unexpected-end-tag");
                        return None;
                    }
                    self.generate_implied_end_tags(None);
                    if !self.current_is(name) {
                        self.error("unclosed-element-at-cell-end");
                    }
                    self.pop_until(name);
                    self.clear_formatting_to_marker();
                    self.mode = Mode::InRow;
                    None
                }
                "body" | "caption" | "col" | "colgroup" | "html" => {
                    self.error("unexpected-end-tag");
                    None
                }
                "table" | "tbody" | "tfoot" | "thead" | "tr" => {
                    if !self.has_in_scope(name, Scope::Table) {
                        self.error("unexpected-end-tag");
                        return None;
                    }
                    self.close_cell();
                    Some(tok)
                }
                _ => self.in_body(tok),
            },
            Token::StartTag { name, .. }
                if matches!(
                    name.as_str(),
                    "caption"
                        | "col"
                        | "colgroup"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) =>
            {
                if !self.has_in_scope_any(&["td", "th"], Scope::Table) {
                    self.error("unexpected-start-tag-in-cell");
                    return None;
                }
                self.close_cell();
                Some(tok)
            }
            _ => self.in_body(tok),
        }
    }
}
