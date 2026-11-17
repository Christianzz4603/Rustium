//! Runs html5lib-format tree-construction `.dat` vectors against `rustium-html`.
//!
//! ```text
//! cargo run -p html5lib-runner -- <dir-with-.dat-files> [-v] [--file NAME] [--limit N]
//! ```
//!
//! Use the debug profile: release builds set `panic = "abort"`, which would turn
//! one panicking test into a crashed run instead of a counted failure.
//!
//! The vectors are maintained in web-platform-tests under
//! `html/syntax/parsing/resources/`. They are not vendored in this repository.
//!
//! Reporting rules (nothing is hidden):
//! - A test that the parser output differs on, or that panics, is a **failure**.
//! - A test the runner cannot execute faithfully is **skipped** and counted
//!   separately: fragment tests whose context element is namespaced
//!   (`svg x`, `math x`), and `#document-fragment` tests are otherwise run.
//! - Parse-error lists (`#errors`) are not compared; only the tree is.

use std::collections::BTreeMap;
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use rustium_dom::{Dom, NodeData, NodeId};
use rustium_html::{parse_document_with, parse_fragment_with, ParseOptions};

struct Case {
    data: String,
    fragment: Option<String>,
    script_off: bool,
    expected: String,
    line: usize,
}

fn parse_dat(src: &str) -> Vec<Case> {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut cases = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i] != "#data" {
            i += 1;
            continue;
        }
        let line = i + 1;
        i += 1;
        let mut data: Vec<&str> = Vec::new();
        while i < lines.len() && lines[i] != "#errors" {
            data.push(lines[i]);
            i += 1;
        }
        let mut fragment = None;
        let mut script_off = false;
        // Skip #errors / #new-errors and read the optional flags until #document.
        while i < lines.len() && lines[i] != "#document" {
            match lines[i] {
                "#document-fragment" => {
                    i += 1;
                    fragment = Some(lines.get(i).copied().unwrap_or("").to_string());
                }
                "#script-off" => script_off = true,
                _ => {}
            }
            i += 1;
        }
        i += 1; // past #document
        let mut doc: Vec<&str> = Vec::new();
        while i < lines.len() {
            if lines[i].is_empty() && lines.get(i + 1) == Some(&"#data") {
                break;
            }
            doc.push(lines[i]);
            i += 1;
        }
        while doc.last() == Some(&"") {
            doc.pop();
        }
        cases.push(Case {
            data: data.join("\n"),
            fragment,
            script_off,
            expected: doc.join("\n"),
            line,
        });
    }
    cases
}

fn dump_node(dom: &Dom, id: NodeId, depth: usize, out: &mut Vec<String>) {
    let pad = "  ".repeat(depth);
    match &dom.node(id).data {
        NodeData::Element(e) => {
            out.push(format!("| {pad}<{}>", e.name));
            let mut attrs: Vec<_> = e.attrs.iter().collect();
            attrs.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
            for a in attrs {
                out.push(format!("| {pad}  {}=\"{}\"", a.name, a.value));
            }
        }
        NodeData::Text(t) => out.push(format!("| {pad}\"{t}\"")),
        NodeData::Comment(c) => out.push(format!("| {pad}<!-- {c} -->")),
        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => {
            if public_id.is_some() || system_id.is_some() {
                out.push(format!(
                    "| {pad}<!DOCTYPE {name} \"{}\" \"{}\">",
                    public_id.as_deref().unwrap_or(""),
                    system_id.as_deref().unwrap_or("")
                ));
            } else {
                out.push(format!("| {pad}<!DOCTYPE {name}>"));
            }
        }
        NodeData::Document | NodeData::DocumentFragment => {}
    }
    for c in dom.children(id) {
        dump_node(dom, c, depth + 1, out);
    }
}

fn dump(dom: &Dom, root: NodeId) -> String {
    let mut out = Vec::new();
    for c in dom.children(root) {
        dump_node(dom, c, 0, &mut out);
    }
    out.join("\n")
}

fn run_case(c: &Case) -> String {
    let opts = ParseOptions {
        scripting: !c.script_off,
    };
    let r = match &c.fragment {
        Some(ctx) => parse_fragment_with(&c.data, ctx, &opts),
        None => parse_document_with(&c.data, &opts),
    };
    dump(&r.dom, r.root)
}

#[derive(Default)]
struct Tally {
    pass: usize,
    fail: usize,
    skip: usize,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut dir: Option<PathBuf> = None;
    let mut verbose = false;
    let mut only: Option<String> = None;
    let mut limit = 5usize;
    while let Some(a) = args.next() {
        match a.as_str() {
            "-v" => verbose = true,
            "--file" => only = args.next(),
            "--limit" => limit = args.next().and_then(|s| s.parse().ok()).unwrap_or(5),
            _ => dir = Some(PathBuf::from(a)),
        }
    }
    let Some(dir) =
        dir.or_else(|| std::env::var_os("RUSTIUM_HTML5LIB_TREE_DIR").map(PathBuf::from))
    else {
        eprintln!("usage: html5lib-runner <dir-with-.dat-files> [-v] [--file NAME] [--limit N]");
        std::process::exit(2);
    };
    let mut files: Vec<PathBuf> = match fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "dat"))
            .collect(),
        Err(e) => {
            eprintln!("cannot read {}: {e}", dir.display());
            std::process::exit(2);
        }
    };
    files.sort();
    std::panic::set_hook(Box::new(|_| {}));
    let mut total = Tally::default();
    let mut per_file: BTreeMap<String, Tally> = BTreeMap::new();
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        if only.as_ref().is_some_and(|o| *o != name) {
            continue;
        }
        let Ok(src) = fs::read_to_string(&f) else {
            eprintln!("skipping unreadable (non-UTF-8?) file {name}");
            continue;
        };
        let mut t = Tally::default();
        let mut shown = 0;
        for c in parse_dat(&src) {
            if c.fragment.as_deref().is_some_and(|ctx| ctx.contains(' ')) {
                t.skip += 1;
                continue;
            }
            let got = catch_unwind(AssertUnwindSafe(|| run_case(&c)));
            match got {
                Ok(g) if g == c.expected => t.pass += 1,
                other => {
                    t.fail += 1;
                    if verbose && shown < limit {
                        shown += 1;
                        println!(
                            "--- FAIL {name}:{} data={:?} fragment={:?}",
                            c.line, c.data, c.fragment
                        );
                        println!("expected:\n{}", c.expected);
                        match other {
                            Ok(g) => println!("got:\n{g}"),
                            Err(_) => println!("got: PANIC"),
                        }
                    }
                }
            }
        }
        total.pass += t.pass;
        total.fail += t.fail;
        total.skip += t.skip;
        per_file.insert(name, t);
    }
    println!("{:<42} {:>6} {:>6} {:>6}", "file", "pass", "fail", "skip");
    for (n, t) in &per_file {
        println!("{n:<42} {:>6} {:>6} {:>6}", t.pass, t.fail, t.skip);
    }
    let ran = total.pass + total.fail;
    println!(
        "TOTAL pass {} / {} run ({:.1}%), skipped {}",
        total.pass,
        ran,
        if ran == 0 {
            0.0
        } else {
            100.0 * total.pass as f64 / ran as f64
        },
        total.skip
    );
}
