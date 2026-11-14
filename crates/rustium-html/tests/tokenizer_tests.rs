#![allow(missing_docs)]
use rustium_html::{tokenize, Token};

fn start(name: &str, attrs: &[(&str, &str)], sc: bool) -> Token {
    Token::StartTag {
        name: name.into(),
        attrs: attrs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect(),
        self_closing: sc,
    }
}
fn end(name: &str) -> Token {
    Token::EndTag { name: name.into() }
}
fn text(s: &str) -> Token {
    Token::Character(s.into())
}

#[test]
fn simple_tags_and_text() {
    assert_eq!(
        tokenize("<p>Hi</p>"),
        vec![start("p", &[], false), text("Hi"), end("p")]
    );
}

#[test]
fn names_are_lowercased() {
    assert_eq!(
        tokenize("<DIV CLASS=A></DIV>"),
        vec![start("div", &[("class", "A")], false), end("div")]
    );
}

#[test]
fn attribute_quoting_styles() {
    assert_eq!(
        tokenize(r#"<a b="1" c='2' d=3 e>"#),
        vec![start(
            "a",
            &[("b", "1"), ("c", "2"), ("d", "3"), ("e", "")],
            false
        )]
    );
}

#[test]
fn attribute_value_may_contain_gt_and_spaces() {
    assert_eq!(
        tokenize(r#"<a title="a > b  c">"#),
        vec![start("a", &[("title", "a > b  c")], false)]
    );
}

#[test]
fn duplicate_attributes_first_wins() {
    assert_eq!(
        tokenize(r#"<a x="1" x="2">"#),
        vec![start("a", &[("x", "1")], false)]
    );
}

#[test]
fn self_closing_flag() {
    assert_eq!(tokenize("<br/>"), vec![start("br", &[], true)]);
    assert_eq!(tokenize("<br />"), vec![start("br", &[], true)]);
}

#[test]
fn character_references_in_text_and_attributes() {
    assert_eq!(
        tokenize("a &amp; b &lt;c&gt; &#65;&#x42;"),
        vec![text("a & b <c> AB")]
    );
    assert_eq!(
        tokenize(r#"<a href="?a=1&b=2&amp;c=3">"#),
        vec![start("a", &[("href", "?a=1&b=2&c=3")], false)]
    );
    // Unknown references stay literal.
    assert_eq!(tokenize("&bogus; &"), vec![text("&bogus; &")]);
}

#[test]
fn comments() {
    assert_eq!(tokenize("<!-- hi -->"), vec![Token::Comment(" hi ".into())]);
    assert_eq!(tokenize("<!---->"), vec![Token::Comment("".into())]);
    assert_eq!(tokenize("<!-->"), vec![Token::Comment("".into())]);
    assert_eq!(tokenize("<!--->"), vec![Token::Comment("".into())]);
    assert_eq!(
        tokenize("<!--a--!>b"),
        vec![Token::Comment("a".into()), text("b")]
    );
    // Unterminated comment runs to EOF.
    assert_eq!(tokenize("<!-- open"), vec![Token::Comment(" open".into())]);
}

#[test]
fn comment_terminator_search_is_bounded_by_first_close() {
    // `--!>` appearing after a proper `-->` must not terminate the earlier comment.
    assert_eq!(
        tokenize("<!--a--><!--b--!>"),
        vec![Token::Comment("a".into()), Token::Comment("b".into())]
    );
    // Many comments: guards the quadratic scan fixed in lex_comment (see benchmarks/).
    let many = "<!--x-->".repeat(20_000);
    assert_eq!(tokenize(&many).len(), 20_000);
}

#[test]
fn bogus_comments() {
    assert_eq!(
        tokenize("<?xml version?>"),
        vec![Token::Comment("?xml version?".into())]
    );
    assert_eq!(tokenize("</ x>"), vec![Token::Comment(" x".into())]);
    assert_eq!(tokenize("<!foo>"), vec![Token::Comment("foo".into())]);
}

#[test]
fn doctype() {
    assert_eq!(
        tokenize("<!DOCTYPE html>"),
        vec![Token::Doctype {
            name: Some("html".into())
        }]
    );
    assert_eq!(
        tokenize("<!doctype HTML>"),
        vec![Token::Doctype {
            name: Some("html".into())
        }]
    );
    assert_eq!(tokenize("<!DOCTYPE>"), vec![Token::Doctype { name: None }]);
    assert_eq!(
        tokenize(r#"<!DOCTYPE html PUBLIC "-//W3C//DTD HTML 4.01//EN">"#),
        vec![Token::Doctype {
            name: Some("html".into())
        }]
    );
}

#[test]
fn stray_less_than_is_text() {
    assert_eq!(tokenize("a < b"), vec![text("a < b")]);
    assert_eq!(tokenize("1<2"), vec![text("1<2")]);
}

#[test]
fn eof_in_tag_drops_tag() {
    assert_eq!(tokenize("x<div class=\"a"), vec![text("x")]);
    assert_eq!(tokenize("x<div"), vec![text("x")]);
}

#[test]
fn rawtext_style_does_not_parse_tags() {
    assert_eq!(
        tokenize("<style>a < b { } <p></style>x"),
        vec![
            start("style", &[], false),
            text("a < b { } <p>"),
            end("style"),
            text("x")
        ]
    );
}

#[test]
fn rcdata_title_decodes_references_but_not_tags() {
    assert_eq!(
        tokenize("<title>a &amp; <b></title>"),
        vec![start("title", &[], false), text("a & <b>"), end("title")]
    );
}

#[test]
fn script_content_is_raw() {
    assert_eq!(
        tokenize("<script>if (a<b && c>d) x = '&amp;';</script>"),
        vec![
            start("script", &[], false),
            text("if (a<b && c>d) x = '&amp;';"),
            end("script")
        ]
    );
}

#[test]
fn raw_end_tag_is_case_insensitive_and_exact() {
    assert_eq!(
        tokenize("<style>a</STYLE >b"),
        vec![
            start("style", &[], false),
            text("a"),
            end("style"),
            text("b")
        ]
    );
    // `</styles>` is not the end tag for `<style>`.
    assert_eq!(
        tokenize("<style>a</styles>b</style>"),
        vec![
            start("style", &[], false),
            text("a</styles>b"),
            end("style")
        ]
    );
}

#[test]
fn plaintext_swallows_everything() {
    assert_eq!(
        tokenize("<plaintext><b>&amp;</b>"),
        // PLAINTEXT does not decode character references.
        vec![start("plaintext", &[], false), text("<b>&amp;</b>")]
    );
}

#[test]
fn null_characters() {
    assert_eq!(tokenize("a\0b"), vec![text("ab")]);
    assert_eq!(
        tokenize("<style>a\0b</style>"),
        vec![start("style", &[], false), text("a\u{FFFD}b"), end("style")]
    );
}

#[test]
fn unicode_is_preserved() {
    assert_eq!(
        tokenize("<p title=\"héllo\">日本語 🎉</p>"),
        vec![
            start("p", &[("title", "héllo")], false),
            text("日本語 🎉"),
            end("p")
        ]
    );
}

#[test]
fn end_tag_with_attributes_and_solidus_recovers() {
    assert_eq!(tokenize("</p class=x>"), vec![end("p")]);
    assert_eq!(tokenize("</br/>"), vec![end("br")]);
    assert_eq!(tokenize("</>"), Vec::<Token>::new());
}

#[test]
fn tokenizer_never_panics_on_garbage() {
    let inputs = [
        "<",
        "<<",
        "</",
        "<!",
        "<!-",
        "<!--",
        "<!DOCTYPE",
        "<a b=",
        "<a b='",
        "&",
        "&#",
        "&#x",
        "&#xZ",
        "<a/",
        "<a /b",
        "<=>",
        "<a =b>",
        "<a \"b\">",
        "\u{0}",
        "<!\u{0}>",
        "<a\u{0}b>",
    ];
    for i in inputs {
        let _ = tokenize(i);
    }
}
