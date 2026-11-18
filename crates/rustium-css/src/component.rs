//! Component values: tokens grouped into simple blocks and functions
//! (CSS Syntax Level 3, "consume a component value").
//!
//! This is the first layer of CSS parsing. It is built with an explicit stack,
//! not recursion, so deeply nested input such as `((((...` cannot overflow the
//! call stack.

use crate::tokenizer::{tokenize, Token};

/// Which bracket pair a simple block uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// `( ... )`
    Paren,
    /// `[ ... ]`
    Square,
    /// `{ ... }`
    Curly,
}

/// A token, or a bracketed group of component values.
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentValue {
    /// A token that is not a bracket opener or a function start.
    Token(Token),
    /// A simple block.
    Block {
        /// Bracket kind.
        kind: BlockKind,
        /// The contents (without the brackets).
        values: Vec<ComponentValue>,
    },
    /// A function: `name(` ... `)`.
    Function {
        /// The function name.
        name: String,
        /// The arguments, unparsed.
        values: Vec<ComponentValue>,
    },
}

impl Drop for ComponentValue {
    /// Drops nested values with a worklist, so a deeply nested tree cannot
    /// overflow the stack through recursive destructors.
    fn drop(&mut self) {
        let mut work: Vec<ComponentValue> = match self {
            ComponentValue::Token(_) => return,
            ComponentValue::Block { values, .. } | ComponentValue::Function { values, .. } => {
                std::mem::take(values)
            }
        };
        while let Some(mut v) = work.pop() {
            if let ComponentValue::Block { values, .. } | ComponentValue::Function { values, .. } =
                &mut v
            {
                work.append(values);
            }
        }
    }
}

/// Component values plus parse errors (tokenizer errors first, then block errors).
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedValues {
    /// The top-level component values.
    pub values: Vec<ComponentValue>,
    /// Spec parse-error names, in the order they were detected.
    pub errors: Vec<&'static str>,
}

enum Opener {
    Block(BlockKind),
    Function(String),
}

struct Frame {
    opener: Opener,
    values: Vec<ComponentValue>,
}

fn closes(opener: &Opener, tok: &Token) -> bool {
    matches!(
        (opener, tok),
        (Opener::Function(_), Token::CloseParen)
            | (Opener::Block(BlockKind::Paren), Token::CloseParen)
            | (Opener::Block(BlockKind::Square), Token::CloseSquare)
            | (Opener::Block(BlockKind::Curly), Token::CloseCurly)
    )
}

fn finish(frame: Frame) -> ComponentValue {
    match frame.opener {
        Opener::Block(kind) => ComponentValue::Block {
            kind,
            values: frame.values,
        },
        Opener::Function(name) => ComponentValue::Function {
            name,
            values: frame.values,
        },
    }
}

/// Groups already-tokenized input into component values.
/// A closing bracket that does not match the innermost open block is kept as a
/// plain token inside it, as the spec's algorithm does. Blocks still open at the
/// end of input are closed and reported as `eof-in-block` / `eof-in-function`.
pub fn component_values(tokens: Vec<Token>) -> ParsedValues {
    let mut errors = Vec::new();
    let mut root: Vec<ComponentValue> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    fn push(root: &mut Vec<ComponentValue>, stack: &mut [Frame], v: ComponentValue) {
        match stack.last_mut() {
            Some(f) => f.values.push(v),
            None => root.push(v),
        }
    }
    for tok in tokens {
        let opener = match &tok {
            Token::OpenParen => Some(Opener::Block(BlockKind::Paren)),
            Token::OpenSquare => Some(Opener::Block(BlockKind::Square)),
            Token::OpenCurly => Some(Opener::Block(BlockKind::Curly)),
            Token::Function(name) => Some(Opener::Function(name.clone())),
            _ => None,
        };
        if let Some(opener) = opener {
            stack.push(Frame {
                opener,
                values: Vec::new(),
            });
            continue;
        }
        if stack.last().is_some_and(|f| closes(&f.opener, &tok)) {
            if let Some(frame) = stack.pop() {
                let v = finish(frame);
                push(&mut root, &mut stack, v);
            }
            continue;
        }
        push(&mut root, &mut stack, ComponentValue::Token(tok));
    }
    while let Some(frame) = stack.pop() {
        errors.push(match frame.opener {
            Opener::Block(_) => "eof-in-block",
            Opener::Function(_) => "eof-in-function",
        });
        let v = finish(frame);
        push(&mut root, &mut stack, v);
    }
    ParsedValues {
        values: root,
        errors,
    }
}

/// Tokenizes `src` and groups it into component values ("parse a list of
/// component values").
pub fn parse_component_value_list(src: &str) -> ParsedValues {
    let t = tokenize(src);
    let mut p = component_values(t.tokens);
    let mut errors = t.errors;
    errors.append(&mut p.errors);
    p.errors = errors;
    p
}
