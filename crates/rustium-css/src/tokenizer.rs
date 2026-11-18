//! The CSS tokenizer itself.

/// Whether a numeric token was written as an integer or with a fraction/exponent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericKind {
    /// No `.` fraction and no exponent.
    Integer,
    /// Has a fraction or exponent.
    Number,
}

/// A CSS token.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// An identifier, with escapes resolved.
    Ident(String),
    /// `name(`; the name has escapes resolved.
    Function(String),
    /// `@name`.
    AtKeyword(String),
    /// `#name`; `is_id` is true when the name would be a valid identifier.
    Hash {
        /// The name after `#`.
        value: String,
        /// Whether it starts like an identifier (type flag "id").
        is_id: bool,
    },
    /// A quoted string, without the quotes.
    String(String),
    /// A string that ended at an unescaped newline.
    BadString(String),
    /// `url(...)` with an unquoted value.
    Url(String),
    /// A malformed unquoted `url(...)`.
    BadUrl,
    /// Any single code point that is not part of another token.
    Delim(char),
    /// A number.
    Number {
        /// The numeric value.
        value: f64,
        /// Integer or not.
        kind: NumericKind,
        /// Whether an explicit `+` or `-` was written.
        has_sign: bool,
    },
    /// A number followed by `%`.
    Percentage {
        /// The value before the `%`.
        value: f64,
        /// Whether an explicit sign was written.
        has_sign: bool,
    },
    /// A number followed by a unit identifier.
    Dimension {
        /// The numeric value.
        value: f64,
        /// Integer or not.
        kind: NumericKind,
        /// Whether an explicit sign was written.
        has_sign: bool,
        /// The unit, with escapes resolved (case preserved).
        unit: String,
    },
    /// One or more whitespace code points.
    Whitespace,
    /// `<!--`.
    Cdo,
    /// `-->`.
    Cdc,
    /// `:`.
    Colon,
    /// `;`.
    Semicolon,
    /// `,`.
    Comma,
    /// `[`.
    OpenSquare,
    /// `]`.
    CloseSquare,
    /// `(`.
    OpenParen,
    /// `)`.
    CloseParen,
    /// `{`.
    OpenCurly,
    /// `}`.
    CloseCurly,
}

/// Tokens plus the parse errors met on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct Tokenized {
    /// The tokens, in order. No end-of-file token is included.
    pub tokens: Vec<Token>,
    /// Spec parse-error names, in the order they occurred.
    pub errors: Vec<&'static str>,
}

struct Lexer {
    c: Vec<char>,
    i: usize,
    errors: Vec<&'static str>,
}

fn preprocess(src: &str) -> Vec<char> {
    let mut out = Vec::with_capacity(src.len());
    let mut it = src.chars().peekable();
    while let Some(ch) = it.next() {
        match ch {
            '\r' => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
                out.push('\n');
            }
            '\u{c}' => out.push('\n'),
            '\0' => out.push('\u{FFFD}'),
            _ => out.push(ch),
        }
    }
    out
}

fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n')
}
fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || !c.is_ascii()
}
fn is_ident(c: char) -> bool {
    is_ident_start(c) || c.is_ascii_digit() || c == '-'
}
fn is_non_printable(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{8}' | '\u{b}' | '\u{e}'..='\u{1f}' | '\u{7f}')
}
fn valid_escape(a: Option<char>, b: Option<char>) -> bool {
    a == Some('\\') && b.is_some_and(|b| b != '\n')
}

impl Lexer {
    fn peek(&self, n: usize) -> Option<char> {
        self.c.get(self.i + n).copied()
    }

    fn err(&mut self, e: &'static str) {
        self.errors.push(e);
    }

    fn starts_ident(&self, at: usize) -> bool {
        let (a, b, c) = (self.peek(at), self.peek(at + 1), self.peek(at + 2));
        match a {
            Some('-') => b.is_some_and(|b| is_ident_start(b) || b == '-') || valid_escape(b, c),
            Some(a) if is_ident_start(a) => true,
            Some('\\') => valid_escape(a, b),
            _ => false,
        }
    }

    fn starts_number(&self, at: usize) -> bool {
        let (a, b, c) = (self.peek(at), self.peek(at + 1), self.peek(at + 2));
        match a {
            Some('+' | '-') => {
                b.is_some_and(|b| b.is_ascii_digit())
                    || (b == Some('.') && c.is_some_and(|c| c.is_ascii_digit()))
            }
            Some('.') => b.is_some_and(|b| b.is_ascii_digit()),
            Some(a) => a.is_ascii_digit(),
            None => false,
        }
    }

    fn consume_comments(&mut self) {
        while self.peek(0) == Some('/') && self.peek(1) == Some('*') {
            self.i += 2;
            loop {
                match self.peek(0) {
                    None => {
                        self.err("eof-in-comment");
                        return;
                    }
                    Some('*') if self.peek(1) == Some('/') => {
                        self.i += 2;
                        break;
                    }
                    Some(_) => self.i += 1,
                }
            }
        }
    }

    /// Consumes an escape; the `\` has been consumed already.
    fn consume_escape(&mut self) -> char {
        let Some(c) = self.peek(0) else {
            self.err("eof-in-escape");
            return '\u{FFFD}';
        };
        self.i += 1;
        if c.is_ascii_hexdigit() {
            let mut v = c.to_digit(16).unwrap_or(0);
            for _ in 0..5 {
                match self.peek(0).and_then(|d| d.to_digit(16)) {
                    Some(d) => {
                        v = v * 16 + d;
                        self.i += 1;
                    }
                    None => break,
                }
            }
            if self.peek(0).is_some_and(is_ws) {
                self.i += 1;
            }
            if v == 0 || (0xD800..=0xDFFF).contains(&v) || v > 0x10FFFF {
                '\u{FFFD}'
            } else {
                char::from_u32(v).unwrap_or('\u{FFFD}')
            }
        } else {
            c
        }
    }

    fn consume_ident_sequence(&mut self) -> String {
        let mut s = String::new();
        loop {
            match self.peek(0) {
                Some(c) if is_ident(c) => {
                    s.push(c);
                    self.i += 1;
                }
                Some('\\') if valid_escape(self.peek(0), self.peek(1)) => {
                    self.i += 1;
                    s.push(self.consume_escape());
                }
                _ => return s,
            }
        }
    }

    fn consume_number(&mut self) -> (f64, NumericKind, bool) {
        let mut repr = String::new();
        let mut kind = NumericKind::Integer;
        let mut has_sign = false;
        if let Some(c @ ('+' | '-')) = self.peek(0) {
            repr.push(c);
            has_sign = true;
            self.i += 1;
        }
        while let Some(c) = self.peek(0).filter(char::is_ascii_digit) {
            repr.push(c);
            self.i += 1;
        }
        if self.peek(0) == Some('.') && self.peek(1).is_some_and(|d| d.is_ascii_digit()) {
            repr.push('.');
            self.i += 1;
            kind = NumericKind::Number;
            while let Some(c) = self.peek(0).filter(char::is_ascii_digit) {
                repr.push(c);
                self.i += 1;
            }
        }
        if matches!(self.peek(0), Some('e' | 'E')) {
            let sign = matches!(self.peek(1), Some('+' | '-'));
            let d = if sign { self.peek(2) } else { self.peek(1) };
            if d.is_some_and(|d| d.is_ascii_digit()) {
                repr.push('e');
                self.i += 1;
                if sign {
                    if let Some(s) = self.peek(0) {
                        repr.push(s);
                    }
                    self.i += 1;
                }
                kind = NumericKind::Number;
                while let Some(c) = self.peek(0).filter(char::is_ascii_digit) {
                    repr.push(c);
                    self.i += 1;
                }
            }
        }
        let value = repr.parse::<f64>().unwrap_or(0.0);
        let value = if value.is_infinite() {
            f64::MAX.copysign(value)
        } else {
            value
        };
        (value, kind, has_sign)
    }

    fn consume_numeric(&mut self) -> Token {
        let (value, kind, has_sign) = self.consume_number();
        if self.starts_ident(0) {
            let unit = self.consume_ident_sequence();
            Token::Dimension {
                value,
                kind,
                has_sign,
                unit,
            }
        } else if self.peek(0) == Some('%') {
            self.i += 1;
            Token::Percentage { value, has_sign }
        } else {
            Token::Number {
                value,
                kind,
                has_sign,
            }
        }
    }

    fn consume_string(&mut self, ending: char) -> Token {
        let mut s = String::new();
        loop {
            match self.peek(0) {
                None => {
                    self.err("eof-in-string");
                    return Token::String(s);
                }
                Some(c) if c == ending => {
                    self.i += 1;
                    return Token::String(s);
                }
                Some('\n') => {
                    self.err("newline-in-string");
                    return Token::BadString(s);
                }
                Some('\\') => match self.peek(1) {
                    None => self.i += 1,
                    Some('\n') => self.i += 2,
                    Some(_) => {
                        self.i += 1;
                        s.push(self.consume_escape());
                    }
                },
                Some(c) => {
                    s.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn consume_bad_url_remnants(&mut self) {
        loop {
            match self.peek(0) {
                None => return,
                Some(')') => {
                    self.i += 1;
                    return;
                }
                Some('\\') if valid_escape(self.peek(0), self.peek(1)) => {
                    self.i += 1;
                    self.consume_escape();
                }
                Some(_) => self.i += 1,
            }
        }
    }

    /// After `url(` when the argument is not a quoted string.
    fn consume_url(&mut self) -> Token {
        while self.peek(0).is_some_and(is_ws) {
            self.i += 1;
        }
        let mut s = String::new();
        loop {
            match self.peek(0) {
                Some(')') => {
                    self.i += 1;
                    return Token::Url(s);
                }
                None => {
                    self.err("eof-in-url");
                    return Token::Url(s);
                }
                Some(c) if is_ws(c) => {
                    while self.peek(0).is_some_and(is_ws) {
                        self.i += 1;
                    }
                    match self.peek(0) {
                        Some(')') => {
                            self.i += 1;
                            return Token::Url(s);
                        }
                        None => {
                            self.err("eof-in-url");
                            return Token::Url(s);
                        }
                        Some(_) => {
                            self.err("whitespace-in-url");
                            self.consume_bad_url_remnants();
                            return Token::BadUrl;
                        }
                    }
                }
                Some('"' | '\'' | '(') => {
                    self.err("bad-character-in-url");
                    self.i += 1;
                    self.consume_bad_url_remnants();
                    return Token::BadUrl;
                }
                Some(c) if is_non_printable(c) => {
                    self.err("bad-character-in-url");
                    self.i += 1;
                    self.consume_bad_url_remnants();
                    return Token::BadUrl;
                }
                Some('\\') => {
                    if valid_escape(self.peek(0), self.peek(1)) {
                        self.i += 1;
                        s.push(self.consume_escape());
                    } else {
                        self.err("bad-escape-in-url");
                        self.i += 1;
                        self.consume_bad_url_remnants();
                        return Token::BadUrl;
                    }
                }
                Some(c) => {
                    s.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn consume_ident_like(&mut self) -> Token {
        let name = self.consume_ident_sequence();
        if self.peek(0) == Some('(') {
            self.i += 1;
            if name.eq_ignore_ascii_case("url") {
                // Whitespace before a quote still makes this an ordinary function.
                let mut k = 0;
                while self.peek(k).is_some_and(is_ws) {
                    k += 1;
                }
                if matches!(self.peek(k), Some('"' | '\'')) {
                    return Token::Function(name);
                }
                return self.consume_url();
            }
            return Token::Function(name);
        }
        Token::Ident(name)
    }

    fn next(&mut self) -> Option<Token> {
        self.consume_comments();
        let c = self.peek(0)?;
        let tok = match c {
            c if is_ws(c) => {
                while self.peek(0).is_some_and(is_ws) {
                    self.i += 1;
                }
                Token::Whitespace
            }
            '"' | '\'' => {
                self.i += 1;
                self.consume_string(c)
            }
            '#' => {
                let n = self.peek(1);
                if n.is_some_and(is_ident) || valid_escape(n, self.peek(2)) {
                    let is_id = self.starts_ident(1);
                    self.i += 1;
                    Token::Hash {
                        value: self.consume_ident_sequence(),
                        is_id,
                    }
                } else {
                    self.i += 1;
                    Token::Delim('#')
                }
            }
            '(' => {
                self.i += 1;
                Token::OpenParen
            }
            ')' => {
                self.i += 1;
                Token::CloseParen
            }
            ',' => {
                self.i += 1;
                Token::Comma
            }
            ':' => {
                self.i += 1;
                Token::Colon
            }
            ';' => {
                self.i += 1;
                Token::Semicolon
            }
            '[' => {
                self.i += 1;
                Token::OpenSquare
            }
            ']' => {
                self.i += 1;
                Token::CloseSquare
            }
            '{' => {
                self.i += 1;
                Token::OpenCurly
            }
            '}' => {
                self.i += 1;
                Token::CloseCurly
            }
            '+' | '.' => {
                if self.starts_number(0) {
                    self.consume_numeric()
                } else {
                    self.i += 1;
                    Token::Delim(c)
                }
            }
            '-' => {
                if self.starts_number(0) {
                    self.consume_numeric()
                } else if self.peek(1) == Some('-') && self.peek(2) == Some('>') {
                    self.i += 3;
                    Token::Cdc
                } else if self.starts_ident(0) {
                    self.consume_ident_like()
                } else {
                    self.i += 1;
                    Token::Delim('-')
                }
            }
            '<' => {
                if self.peek(1) == Some('!')
                    && self.peek(2) == Some('-')
                    && self.peek(3) == Some('-')
                {
                    self.i += 4;
                    Token::Cdo
                } else {
                    self.i += 1;
                    Token::Delim('<')
                }
            }
            '@' => {
                if self.starts_ident(1) {
                    self.i += 1;
                    Token::AtKeyword(self.consume_ident_sequence())
                } else {
                    self.i += 1;
                    Token::Delim('@')
                }
            }
            '\\' => {
                if valid_escape(self.peek(0), self.peek(1)) {
                    self.consume_ident_like()
                } else {
                    self.err("invalid-escape");
                    self.i += 1;
                    Token::Delim('\\')
                }
            }
            c if c.is_ascii_digit() => self.consume_numeric(),
            c if is_ident_start(c) => self.consume_ident_like(),
            c => {
                self.i += 1;
                Token::Delim(c)
            }
        };
        Some(tok)
    }
}

/// Tokenizes a stylesheet or fragment of CSS.
pub fn tokenize(src: &str) -> Tokenized {
    let mut lx = Lexer {
        c: preprocess(src),
        i: 0,
        errors: Vec::new(),
    };
    let mut tokens = Vec::new();
    while let Some(t) = lx.next() {
        tokens.push(t);
    }
    Tokenized {
        tokens,
        errors: lx.errors,
    }
}
