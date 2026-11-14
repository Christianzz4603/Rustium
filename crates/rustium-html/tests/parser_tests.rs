#![allow(missing_docs)]

use rustium_common::Severity;
use rustium_dom::{Dom, NodeData, NodeId};
use rustium_html::{parse_document, parse_document_with, parse_fragment, ParseOptions};

/// Full document serialization.
fn doc(input: &str) -> String {
    let r = parse_document(input);
    r.dom.inner_html(r.root)
}

/// Serialization of the children of <body>.
fn body(input: &str) -> String {
    let r = parse_document(input);
    let b = r.dom.get_elements_by_tag_name(r.root, "body");
    assert_eq!(b.len(), 1, "expected exactly one body for {input:?}");
    r.dom.inner_html(b[0])
}

// ---------- document structure ----------

#[test]
fn empty_input_creates_skeleton() {
    assert_eq!(doc(""), "<html><head></head><body></body></html>");
}

#[test]
fn bare_text_creates_skeleton() {
    assert_eq!(doc("Hello"), "<html><head></head><body>Hello</body></html>");
}

#[test]
fn full_document_round_trips() {
    let s = "<!DOCTYPE html><html><head><title>T</title></head><body><p>x</p></body></html>";
    assert_eq!(doc(s), s);
}

#[test]
fn whitespace_between_structural_elements() {
    assert_eq!(
        doc("<html> <head> </head> <body> </body> </html>"),
        "<html><head> </head> <body>  </body></html>"
    );
}

#[test]
fn comments_in_every_position() {
    assert_eq!(
        doc("<!--a--><html><!--b--><body><!--c--></body></html><!--d-->"),
        "<!--a--><html><!--b--><head></head><body><!--c--></body></html><!--d-->"
    );
}

#[test]
fn head_elements_after_head_go_into_head() {
    assert_eq!(
        doc("<head></head><meta charset=x><body>"),
        "<html><head><meta charset=\"x\"></head><body></body></html>"
    );
    assert_eq!(
        doc("<head></head><title>t</title><body>"),
        "<html><head><title>t</title></head><body></body></html>"
    );
}

#[test]
fn content_after_body_end_is_still_in_body() {
    assert_eq!(body("<body>a</body>b"), "ab");
    assert_eq!(body("<html><body>a</body></html>b"), "ab");
}

#[test]
fn html_and_body_attributes_merge() {
    assert_eq!(
        doc("<html a=1><body b=2><html c=3 a=9><body d=4>"),
        "<html a=\"1\" c=\"3\"><head></head><body b=\"2\" d=\"4\"></body></html>"
    );
}

#[test]
fn doctype_node_is_created() {
    let r = parse_document("<!DOCTYPE html><p>");
    let first = r.dom.children(r.root).next().unwrap();
    assert_eq!(
        r.dom.node(first).data,
        NodeData::Doctype {
            name: "html".into()
        }
    );
}

// ---------- implied end tags ----------

#[test]
fn paragraphs_close_each_other() {
    assert_eq!(body("<p>a<p>b"), "<p>a</p><p>b</p>");
}

#[test]
fn block_closes_open_paragraph() {
    assert_eq!(body("<p>a<div>b</div>"), "<p>a</p><div>b</div>");
}

#[test]
fn list_items_close_each_other() {
    assert_eq!(body("<ul><li>a<li>b</ul>"), "<ul><li>a</li><li>b</li></ul>");
}

#[test]
fn definition_lists() {
    assert_eq!(
        body("<dl><dt>a<dd>b<dt>c</dl>"),
        "<dl><dt>a</dt><dd>b</dd><dt>c</dt></dl>"
    );
}

#[test]
fn nested_lists_do_not_close_outer_items() {
    assert_eq!(
        body("<ul><li>a<ul><li>b</ul><li>c</ul>"),
        "<ul><li>a<ul><li>b</li></ul></li><li>c</li></ul>"
    );
}

#[test]
fn headings_do_not_nest() {
    assert_eq!(body("<h1>a<h2>b"), "<h1>a</h1><h2>b</h2>");
}

#[test]
fn options_close_each_other() {
    assert_eq!(
        body("<select><option>a<option>b</select>"),
        "<select><option>a</option><option>b</option></select>"
    );
}

#[test]
fn buttons_do_not_nest() {
    assert_eq!(
        body("<button>a<button>b"),
        "<button>a</button><button>b</button>"
    );
}

#[test]
fn nested_forms_are_ignored() {
    assert_eq!(body("<form><form>"), "<form></form>");
}

// ---------- stray and special tags ----------

#[test]
fn stray_end_tags_are_ignored() {
    assert_eq!(body("a</div>b"), "ab");
    assert_eq!(body("a</span>b"), "ab");
}

#[test]
fn stray_end_p_creates_empty_paragraph() {
    assert_eq!(body("x</p>"), "x<p></p>");
}

#[test]
fn end_br_acts_like_br() {
    assert_eq!(body("x</br>y"), "x<br>y");
}

#[test]
fn void_elements_have_no_children() {
    assert_eq!(
        body("<br><img src=x><input>text"),
        "<br><img src=\"x\"><input>text"
    );
}

#[test]
fn self_closing_syntax_on_non_void_is_ignored() {
    assert_eq!(body("<p/>x"), "<p>x</p>");
    let r = parse_document("<div/>");
    assert!(r
        .diagnostics
        .iter()
        .any(|d| d.code == "non-void-html-element-start-tag-with-trailing-solidus"));
}

#[test]
fn image_becomes_img() {
    assert_eq!(body("<image src=a>"), "<img src=\"a\">");
}

#[test]
fn pre_and_textarea_drop_first_newline() {
    assert_eq!(body("<pre>\nx</pre>"), "<pre>x</pre>");
    assert_eq!(body("<pre>\n\nx</pre>"), "<pre>\nx</pre>");
    let r = parse_document("<textarea>\nx</textarea>");
    let t = r.dom.get_elements_by_tag_name(r.root, "textarea")[0];
    assert_eq!(r.dom.text_content(t), "x");
}

#[test]
fn malformed_attributes() {
    assert_eq!(
        body("<div class=\"a\"id=b>"),
        "<div class=\"a\" id=\"b\"></div>"
    );
}

// ---------- raw text and entities ----------

#[test]
fn script_and_style_content_is_not_parsed() {
    let r = parse_document("<script>a<b</script><style>p>q{}</style>");
    let s = r.dom.get_elements_by_tag_name(r.root, "script")[0];
    let st = r.dom.get_elements_by_tag_name(r.root, "style")[0];
    assert_eq!(r.dom.text_content(s), "a<b");
    assert_eq!(r.dom.text_content(st), "p>q{}");
    assert!(r.dom.get_elements_by_tag_name(r.root, "b").is_empty());
}

#[test]
fn unterminated_script_runs_to_eof() {
    assert_eq!(
        doc("<script>abc"),
        "<html><head><script>abc</script></head><body></body></html>"
    );
}

#[test]
fn entities_in_body_text() {
    let r = parse_document("<p>&amp;&lt;&copy;&#65;</p>");
    let p = r.dom.get_elements_by_tag_name(r.root, "p")[0];
    assert_eq!(r.dom.text_content(p), "&<\u{a9}A");
}

#[test]
fn noscript_depends_on_scripting_option() {
    let on = parse_document("x<noscript><p>y</p></noscript>");
    let n = on.dom.get_elements_by_tag_name(on.root, "noscript")[0];
    assert_eq!(on.dom.text_content(n), "<p>y</p>");
    assert!(on.dom.get_elements_by_tag_name(on.root, "p").is_empty());

    let off = parse_document_with(
        "x<noscript><p>y</p></noscript>",
        &ParseOptions { scripting: false },
    );
    let b = off.dom.get_elements_by_tag_name(off.root, "body")[0];
    assert_eq!(off.dom.inner_html(b), "x<noscript><p>y</p></noscript>");
}

#[test]
fn crlf_is_normalized() {
    let r = parse_document("<pre>a\r\nb\rc</pre>");
    let p = r.dom.get_elements_by_tag_name(r.root, "pre")[0];
    assert_eq!(r.dom.text_content(p), "a\nb\nc");
}

// ---------- formatting elements / adoption agency ----------

#[test]
fn misnested_bold_italic() {
    assert_eq!(body("<b><i>x</b>y</i>"), "<b><i>x</i></b><i>y</i>");
}

#[test]
fn misnested_inside_paragraph() {
    assert_eq!(
        body("<p>1<b>2<i>3</b>4</i>5</p>"),
        "<p>1<b>2<i>3</i></b><i>4</i>5</p>"
    );
}

#[test]
fn anchor_around_block() {
    assert_eq!(
        body("<a>1<div>2</a>3</div>"),
        "<a>1</a><div><a>2</a>3</div>"
    );
}

#[test]
fn nested_anchors_split() {
    assert_eq!(body("<a>1<a>2"), "<a>1</a><a>2</a>");
}

#[test]
fn formatting_reopens_across_paragraphs() {
    assert_eq!(body("<p><b>a<p>b"), "<p><b>a</b></p><p><b>b</b></p>");
}

#[test]
fn unclosed_formatting_at_eof() {
    assert_eq!(body("<p>1<b>2<i>3"), "<p>1<b>2<i>3</i></b></p>");
}

// ---------- tables (simplified) ----------

#[test]
fn simplified_table_structure() {
    assert_eq!(
        body("<table><tr><td>a<td>b<tr><td>c</table>x"),
        "<table><tr><td>a</td><td>b</td></tr><tr><td>c</td></tr></table>x"
    );
}

#[test]
fn table_reports_unsupported_diagnostic() {
    let r = parse_document("<table><tr><td>a</table>");
    assert!(r
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Unsupported
            && d.code == "table-simplified-tree-construction"));
}

#[test]
fn table_formatting_does_not_leak_out_of_cells() {
    assert_eq!(
        body("<table><tr><td><b>a</table>c"),
        "<table><tr><td><b>a</b></td></tr></table>c"
    );
}

// ---------- fragments ----------

#[test]
fn fragment_with_list_context() {
    let r = parse_fragment("<li>a<li>b", "ul");
    assert_eq!(r.dom.inner_html(r.root), "<li>a</li><li>b</li>");
    assert!(r.dom.node(r.root).parent.is_none());
    assert!(matches!(
        r.dom.node(r.root).data,
        NodeData::DocumentFragment
    ));
    assert_eq!(r.dom.children(r.dom.document()).count(), 0);
}

#[test]
fn fragment_recovers_markup() {
    let r = parse_fragment("a<b>c", "div");
    assert_eq!(r.dom.inner_html(r.root), "a<b>c</b>");
}

#[test]
fn fragment_textarea_context_is_rcdata() {
    let r = parse_fragment("x &amp; <b>y", "textarea");
    assert_eq!(r.dom.text_content(r.root), "x & <b>y");
}

// ---------- diagnostics ----------

#[test]
fn valid_document_has_no_parse_errors() {
    let r = parse_document(
        "<!DOCTYPE html><html><head><title>x</title></head><body><p>hi</p></body></html>",
    );
    let errs: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::ParseError)
        .collect();
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn missing_doctype_is_reported() {
    let r = parse_document("<p>hi");
    assert!(r.diagnostics.iter().any(|d| d.code == "missing-doctype"));
}

#[test]
fn diagnostics_are_ordered_by_offset() {
    let r = parse_document("<p>a</b></i><br/>&#0;");
    let offs: Vec<usize> = r.diagnostics.iter().map(|d| d.span.start).collect();
    let mut sorted = offs.clone();
    sorted.sort();
    assert_eq!(offs, sorted);
    assert!(!offs.is_empty());
}

// ---------- limits and robustness ----------

#[test]
fn deep_nesting_is_bounded_and_does_not_overflow() {
    let n = 5000;
    let html = "<div>".repeat(n);
    let r = parse_document(&html);
    assert_eq!(r.dom.get_elements_by_tag_name(r.root, "div").len(), n);
    assert!(r
        .diagnostics
        .iter()
        .any(|d| d.code == "max-open-element-depth-exceeded"));
    // Serialization recurses over the tree; it must survive the cap.
    let s = r.dom.inner_html(r.root);
    assert!(s.starts_with("<html>"));
}

#[test]
fn many_unclosed_formatting_elements_stay_linear() {
    // Regression: past the depth cap, reconstruction used to re-create every
    // unclosed formatting element on each text token (6M elements for 3000 tags).
    let html = "<b>x".repeat(3000);
    let r = parse_document(&html);
    assert_eq!(r.dom.get_elements_by_tag_name(r.root, "b").len(), 3000);
    // Distinct attributes defeat the Noah's Ark clause; the list cap must still hold.
    let distinct: String = (0..3000).map(|i| format!("<b class=c{i}>x")).collect();
    let r = parse_document(&distinct);
    assert!(r.dom.len() < 20_000, "node count exploded: {}", r.dom.len());
}

#[test]
fn noahs_ark_limits_identical_formatting_reconstruction() {
    // Four identical <b> are open, then </p>-style closure forces reconstruction.
    let r = parse_document("<p><b><b><b><b>a</p>c");
    let n = r.dom.get_elements_by_tag_name(r.root, "b").len();
    // 4 original + at most 3 reconstructed for "c".
    assert_eq!(n, 4 + 3);
}

fn check_tree(dom: &Dom) {
    let doc = dom.document();
    let mut all: Vec<NodeId> = vec![doc];
    all.extend(dom.descendants(doc));
    let elements_in_doc = dom
        .children(doc)
        .filter(|&c| matches!(dom.node(c).data, NodeData::Element(_)))
        .count();
    assert!(
        elements_in_doc <= 1,
        "document has {elements_in_doc} element children"
    );
    for n in all {
        let kids: Vec<NodeId> = dom.children(n).collect();
        assert_eq!(dom.node(n).first_child, kids.first().copied());
        assert_eq!(dom.node(n).last_child, kids.last().copied());
        for (i, &k) in kids.iter().enumerate() {
            let node = dom.node(k);
            assert_eq!(node.parent, Some(n));
            assert_eq!(
                node.prev_sibling,
                if i == 0 { None } else { Some(kids[i - 1]) }
            );
            assert_eq!(node.next_sibling, kids.get(i + 1).copied());
        }
        if matches!(dom.node(n).data, NodeData::Document) {
            assert!(!kids
                .iter()
                .any(|&k| matches!(dom.node(k).data, NodeData::Text(_))));
        }
    }
}

#[test]
fn seeded_fuzz_keeps_tree_invariants() {
    const ALPHABET: &[&str] = &[
        "<b>",
        "</b>",
        "<i>",
        "</i>",
        "<p>",
        "</p>",
        "<div>",
        "</div>",
        "<a>",
        "</a>",
        "<table>",
        "<tr>",
        "<td>",
        "</td>",
        "</table>",
        "<li>",
        "<ul>",
        "</ul>",
        "<h1>",
        "</h1>",
        "x",
        " ",
        "&amp;",
        "<br>",
        "<!-- c -->",
        "<script>",
        "</script>",
        "<style>",
        "</style>",
        "<title>",
        "</title>",
        "<select>",
        "<option>",
        "<form>",
        "</form>",
        "<button>",
        "<nobr>",
        "</nobr>",
        "<body>",
        "</body>",
        "</html>",
        "<html>",
        "<head>",
        "</head>",
        "<textarea>",
        "</textarea>",
        "<pre>",
        "<svg>",
        "<template>",
        "</template>",
        "<frameset>",
        "<dd>",
        "<dt>",
        "<font>",
        "</font>",
        "<em>",
        "</em>",
        "<!DOCTYPE html>",
        "<",
        ">",
        "&",
        "\n",
        "\0",
    ];
    let mut state: u64 = 0x2545F4914F6CDD1D;
    for _ in 0..3000 {
        let mut s = String::new();
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let len = 1 + (state >> 59) as usize * 2;
        for _ in 0..len {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            s.push_str(ALPHABET[(state >> 33) as usize % ALPHABET.len()]);
        }
        let r = parse_document(&s);
        check_tree(&r.dom);
        let _ = r.dom.inner_html(r.root);
        let f = parse_fragment(&s, "div");
        check_tree(&f.dom);
    }
}
