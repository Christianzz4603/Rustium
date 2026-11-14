//! Baseline HTML parsing benchmark.
//!
//! Run with `cargo run --release -p parse-bench`. Numbers are a baseline for
//! later optimization work, not a comparison with other engines. Memory is not
//! measured yet (no allocator hooks); node counts are reported instead.

use std::time::Instant;

fn build_document(sections: usize) -> String {
    let mut s = String::from(
        "<!DOCTYPE html><html><head><title>Bench</title><style>p{color:red}</style></head><body>",
    );
    for i in 0..sections {
        s.push_str(&format!(
            "<div class=\"card c{i}\" id=\"s{i}\"><h2>Section {i}</h2>\
             <p>Some <b>bold</b>, <i>italic</i> and <a href=\"/p/{i}?a=1&amp;b=2\">linked</a> text &amp; entities &copy; 2026.</p>\
             <ul><li>one<li>two<li>three</ul>\
             <p>Misnested <b>bold <i>both</b> italic</i> tail.\
             <table><tr><td>a<td>b<tr><td>c<td>d</table>\
             <img src=\"x{i}.png\" alt=\"pic\"><br><!-- comment {i} --></div>\n"
        ));
    }
    s.push_str("</body></html>");
    s
}

fn main() {
    println!("rustium parse-bench (release builds only are meaningful)");
    println!(
        "{:<12} {:>10} {:>10} {:>10} {:>12}",
        "size", "iters", "ms/parse", "MB/s", "nodes"
    );
    for &sections in &[10usize, 100, 1000, 5000] {
        let html = build_document(sections);
        let bytes = html.len();
        let iters = (50_000_000 / bytes).clamp(3, 500);
        // Warm-up.
        let nodes = rustium_html::parse_document(&html).dom.len();
        let start = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(rustium_html::parse_document(std::hint::black_box(&html)));
        }
        let secs = start.elapsed().as_secs_f64();
        let per = secs / iters as f64;
        println!(
            "{:<12} {:>10} {:>10.3} {:>10.1} {:>12}",
            format!("{} KB", bytes / 1024),
            iters,
            per * 1000.0,
            bytes as f64 / per / 1_000_000.0,
            nodes
        );
    }

    // Startup: first parse of a tiny document in a fresh process.
    let t = Instant::now();
    std::hint::black_box(rustium_html::parse_document("<p>hi"));
    println!("first tiny parse: {:?}", t.elapsed());
}
