#![allow(missing_docs)]
use rustium_css::{tokenize, NumericKind, Token};

fn toks(s: &str) -> Vec<Token> {
    tokenize(s).tokens
}
fn ident(s: &str) -> Token {
    Token::Ident(s.into())
}
fn num(v: f64, kind: NumericKind, has_sign: bool) -> Token {
    Token::Number {
        value: v,
        kind,
        has_sign,
    }
}

#[test]
fn simple_rule() {
    assert_eq!(
        toks("a{color:red}"),
        vec![
            ident("a"),
            Token::OpenCurly,
            ident("color"),
            Token::Colon,
            ident("red"),
            Token::CloseCurly
        ]
    );
}

#[test]
fn whitespace_is_collapsed_and_comments_vanish() {
    assert_eq!(
        toks("a /* x */\n\t b"),
        vec![ident("a"), Token::Whitespace, Token::Whitespace, ident("b")]
    );
    // Two comments in a row, then the token.
    assert_eq!(toks("/**//**/a"), vec![ident("a")]);
}

#[test]
fn unterminated_comment_is_an_error_not_a_token() {
    let t = tokenize("a/* oops");
    assert_eq!(t.tokens, vec![ident("a")]);
    assert_eq!(t.errors, vec!["eof-in-comment"]);
}

#[test]
fn numbers() {
    use NumericKind::{Integer, Number};
    assert_eq!(toks("12"), vec![num(12.0, Integer, false)]);
    assert_eq!(toks("+12"), vec![num(12.0, Integer, true)]);
    assert_eq!(toks("-1.5"), vec![num(-1.5, Number, true)]);
    assert_eq!(toks(".5"), vec![num(0.5, Number, false)]);
    assert_eq!(toks("1e3"), vec![num(1000.0, Number, false)]);
    assert_eq!(toks("1E-2"), vec![num(0.01, Number, false)]);
    // `1e` is a dimension with unit `e`, not an exponent.
    assert_eq!(
        toks("1e"),
        vec![Token::Dimension {
            value: 1.0,
            kind: Integer,
            has_sign: false,
            unit: "e".into()
        }]
    );
    // A trailing dot is a delimiter.
    assert_eq!(
        toks("1."),
        vec![num(1.0, Integer, false), Token::Delim('.')]
    );
}

#[test]
fn huge_numbers_are_clamped_not_infinite() {
    match &toks("1e999")[0] {
        Token::Number { value, .. } => assert_eq!(*value, f64::MAX),
        t => panic!("{t:?}"),
    }
    match &toks("-1e999")[0] {
        Token::Number { value, .. } => assert_eq!(*value, -f64::MAX),
        t => panic!("{t:?}"),
    }
}

#[test]
fn dimensions_and_percentages() {
    assert_eq!(
        toks("10px"),
        vec![Token::Dimension {
            value: 10.0,
            kind: NumericKind::Integer,
            has_sign: false,
            unit: "px".into()
        }]
    );
    assert_eq!(
        toks("50%"),
        vec![Token::Percentage {
            value: 50.0,
            has_sign: false
        }]
    );
    assert_eq!(
        toks("-2.5em"),
        vec![Token::Dimension {
            value: -2.5,
            kind: NumericKind::Number,
            has_sign: true,
            unit: "em".into()
        }]
    );
}

#[test]
fn idents_functions_at_keywords_and_hashes() {
    assert_eq!(toks("--custom"), vec![ident("--custom")]);
    assert_eq!(toks("-moz-x"), vec![ident("-moz-x")]);
    assert_eq!(toks("rgb("), vec![Token::Function("rgb".into())]);
    assert_eq!(toks("@media"), vec![Token::AtKeyword("media".into())]);
    assert_eq!(
        toks("@ media"),
        vec![Token::Delim('@'), Token::Whitespace, ident("media")]
    );
    assert_eq!(
        toks("#id"),
        vec![Token::Hash {
            value: "id".into(),
            is_id: true
        }]
    );
    // A hash starting with a digit is valid but not an id.
    assert_eq!(
        toks("#1a"),
        vec![Token::Hash {
            value: "1a".into(),
            is_id: false
        }]
    );
    assert_eq!(toks("# "), vec![Token::Delim('#'), Token::Whitespace]);
}

#[test]
fn escapes_in_identifiers() {
    assert_eq!(toks(r"\31 23"), vec![ident("123")]);
    assert_eq!(toks(r"a\62 c"), vec![ident("abc")]);
    assert_eq!(toks(r"\{x"), vec![ident("{x")]);
    // Out of range, surrogate and NUL escapes become U+FFFD.
    assert_eq!(toks(r"\110000 "), vec![ident("\u{FFFD}")]);
    assert_eq!(toks(r"\d800 "), vec![ident("\u{FFFD}")]);
    assert_eq!(toks(r"\0 "), vec![ident("\u{FFFD}")]);
    // A backslash before a newline is not an escape.
    let t = tokenize("\\\n");
    assert_eq!(t.tokens, vec![Token::Delim('\\'), Token::Whitespace]);
    assert_eq!(t.errors, vec!["invalid-escape"]);
}

#[test]
fn strings() {
    assert_eq!(toks("\"a b\""), vec![Token::String("a b".into())]);
    assert_eq!(toks("'a\"b'"), vec![Token::String("a\"b".into())]);
    // An escaped newline continues the string and vanishes.
    assert_eq!(toks("\"a\\\nb\""), vec![Token::String("ab".into())]);
    assert_eq!(toks(r#""\41 \"""#), vec![Token::String("A\"".into())]);
}

#[test]
fn bad_and_unterminated_strings() {
    let t = tokenize("\"ab\ncd");
    assert_eq!(
        t.tokens,
        vec![
            Token::BadString("ab".into()),
            Token::Whitespace,
            ident("cd")
        ]
    );
    assert_eq!(t.errors, vec!["newline-in-string"]);
    let t = tokenize("\"ab");
    assert_eq!(t.tokens, vec![Token::String("ab".into())]);
    assert_eq!(t.errors, vec!["eof-in-string"]);
}

#[test]
fn urls() {
    assert_eq!(toks("url(a.png)"), vec![Token::Url("a.png".into())]);
    assert_eq!(toks("url(  a.png  )"), vec![Token::Url("a.png".into())]);
    assert_eq!(toks("URL(a)"), vec![Token::Url("a".into())]);
    assert_eq!(toks(r"url(a\29 b)"), vec![Token::Url("a)b".into())]);
    // Quoted url is an ordinary function token followed by a string.
    assert_eq!(
        toks("url(\"a\")"),
        vec![
            Token::Function("url".into()),
            Token::String("a".into()),
            Token::CloseParen
        ]
    );
    assert_eq!(
        toks("url( 'a')"),
        vec![
            Token::Function("url".into()),
            Token::Whitespace,
            Token::String("a".into()),
            Token::CloseParen
        ]
    );
    // `url` without a paren is an ident.
    assert_eq!(toks("url"), vec![ident("url")]);
}

#[test]
fn bad_urls_consume_to_the_closing_paren() {
    let t = tokenize("url(a b)c");
    assert_eq!(t.tokens, vec![Token::BadUrl, ident("c")]);
    assert_eq!(t.errors, vec!["whitespace-in-url"]);
    let t = tokenize("url(a\"b)c");
    assert_eq!(t.tokens, vec![Token::BadUrl, ident("c")]);
    assert_eq!(t.errors, vec!["bad-character-in-url"]);
    // An escaped paren inside the remnants does not end the bad url.
    assert_eq!(toks(r"url(a b\) c) d")[0], Token::BadUrl);
    let t = tokenize("url(a");
    assert_eq!(t.tokens, vec![Token::Url("a".into())]);
    assert_eq!(t.errors, vec!["eof-in-url"]);
}

#[test]
fn cdo_cdc_and_delims() {
    assert_eq!(
        toks("<!-- -->"),
        vec![Token::Cdo, Token::Whitespace, Token::Cdc]
    );
    assert_eq!(
        toks("<!-"),
        vec![Token::Delim('<'), Token::Delim('!'), Token::Delim('-')]
    );
    assert_eq!(toks("a-->"), vec![ident("a--"), Token::Delim('>')]);
    assert_eq!(toks("+"), vec![Token::Delim('+')]);
    assert_eq!(toks("-"), vec![Token::Delim('-')]);
    assert_eq!(toks("*"), vec![Token::Delim('*')]);
}

#[test]
fn punctuation() {
    assert_eq!(
        toks("(),:;[]{}"),
        vec![
            Token::OpenParen,
            Token::CloseParen,
            Token::Comma,
            Token::Colon,
            Token::Semicolon,
            Token::OpenSquare,
            Token::CloseSquare,
            Token::OpenCurly,
            Token::CloseCurly
        ]
    );
}

#[test]
fn preprocessing_normalizes_newlines_and_nul() {
    assert_eq!(
        toks("a\r\nb"),
        vec![ident("a"), Token::Whitespace, ident("b")]
    );
    assert_eq!(
        toks("a\u{c}b"),
        vec![ident("a"), Token::Whitespace, ident("b")]
    );
    assert_eq!(toks("a\0b"), vec![ident("a\u{FFFD}b")]);
    // CR LF inside a string is a newline, so the string is bad.
    assert!(matches!(toks("\"a\r\nb\"")[0], Token::BadString(_)));
}

#[test]
fn non_ascii_is_ident() {
    assert_eq!(toks("é"), vec![ident("é")]);
    assert_eq!(toks("a😀"), vec![ident("a😀")]);
}

#[test]
fn a_realistic_declaration() {
    let t = tokenize("margin: -0.5em auto !important;");
    assert!(t.errors.is_empty());
    assert_eq!(t.tokens.len(), 10);
    assert_eq!(t.tokens[0], ident("margin"));
    assert_eq!(t.tokens[7], Token::Delim('!'));
}

#[test]
fn seeded_fuzz_never_panics_and_always_terminates() {
    const PIECES: &[&str] = &[
        "a", "-", "--", "+", ".", "1", "e", "%", "px", "\\", "\\31 ", "\"", "'", "\n", " ", "/*",
        "*/", "/", "(", ")", "{", "}", "[", "]", ";", ":", ",", "#", "@", "<!--", "-->", "url(",
        "url(\"", "\0", "é", "9", "E+", "\r\n", "!",
    ];
    let mut state: u64 = 0x9E3779B97F4A7C15;
    for _ in 0..4000 {
        let mut s = String::new();
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let len = 1 + (state >> 58) as usize * 2;
        for _ in 0..len {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            s.push_str(PIECES[(state >> 33) as usize % PIECES.len()]);
        }
        let t = tokenize(&s);
        // Every token consumes at least one input character.
        assert!(t.tokens.len() <= s.chars().count() + 1, "input {s:?}");
    }
}

#[test]
fn long_inputs_stay_linear() {
    // 200k tokens must not take quadratic time (this finishes instantly if linear).
    let s = "a{b:c;}".repeat(30_000);
    assert_eq!(tokenize(&s).tokens.len(), 30_000 * 7);
    // An unterminated comment full of `*` characters (never followed by `/`).
    let s = format!("/*{}", "x*".repeat(50_000));
    let t = tokenize(&s);
    assert!(t.tokens.is_empty());
    assert_eq!(t.errors, vec!["eof-in-comment"]);
}
