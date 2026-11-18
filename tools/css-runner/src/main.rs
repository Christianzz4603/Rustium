//! Runs `css-parsing-tests` (<https://github.com/SimonSapin/css-parsing-tests>)
//! vectors against `rustium-css`. The vectors are not vendored.
//!
//! ```text
//! cargo run -p css-runner -- <dir-with-json> [-v] [--file NAME]
//! ```
//!
//! Use the debug profile (release is `panic = "abort"`).
//!
//! Supported files: `component_value_list.json`.
//!
//! Reporting rules (nothing is hidden):
//! - A differing result, or a panic, is a **failure**.
//! - A vector this implementation cannot match by design is **skipped** and
//!   counted, with the reason: `unicode-range` tokens (an older spec revision
//!   had a `U+` token; the current tokenizer has none), the `~=`-style match
//!   tokens (also removed from the spec), and vectors expecting `["error", ")"]`
//!   style entries for mismatched closers (not produced by this layer).
//! - Numbers are compared by value and integer/number type only: the tokenizer
//!   does not keep the source text ("repr") of a number. Percentages are
//!   compared by value only (the token keeps no integer/number type).
//! - Of the parse errors, only `eof-in-string` and `eof-in-url` are compared
//!   (as counts); the vectors list no others except `bad-string` / `bad-url`,
//!   which are tokens here.

use std::collections::BTreeMap;
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

#[path = "../../shared/json.rs"]
mod json;
use json::{Json, JsonParser};

use rustium_css::{parse_component_value_list, BlockKind, ComponentValue, NumericKind, Token};

fn s(x: &str) -> Json {
    Json::Str(x.to_string())
}

fn kind(k: NumericKind) -> Json {
    s(match k {
        NumericKind::Integer => "integer",
        NumericKind::Number => "number",
    })
}

fn token(t: &Token) -> Json {
    let arr = |v: Vec<Json>| Json::Arr(v);
    match t {
        Token::Ident(v) => arr(vec![s("ident"), s(v)]),
        Token::AtKeyword(v) => arr(vec![s("at-keyword"), s(v)]),
        Token::Hash { value, is_id } => arr(vec![
            s("hash"),
            s(value),
            s(if *is_id { "id" } else { "unrestricted" }),
        ]),
        Token::String(v) => arr(vec![s("string"), s(v)]),
        Token::BadString(_) => arr(vec![s("error"), s("bad-string")]),
        Token::Url(v) => arr(vec![s("url"), s(v)]),
        Token::BadUrl => arr(vec![s("error"), s("bad-url")]),
        Token::Delim(c) => s(&c.to_string()),
        Token::Number { value, kind: k, .. } => {
            arr(vec![s("number"), s(""), Json::Num(*value), kind(*k)])
        }
        Token::Percentage { value, .. } => {
            arr(vec![s("percentage"), s(""), Json::Num(*value), s("")])
        }
        Token::Dimension {
            value,
            kind: k,
            unit,
            ..
        } => arr(vec![
            s("dimension"),
            s(""),
            Json::Num(*value),
            kind(*k),
            s(unit),
        ]),
        Token::Whitespace => s(" "),
        Token::Cdo => s("<!--"),
        Token::Cdc => s("-->"),
        Token::Colon => s(":"),
        Token::Semicolon => s(";"),
        Token::Comma => s(","),
        // Bracket tokens are consumed by the component-value layer.
        other => s(&format!("?{other:?}")),
    }
}

fn value(v: &ComponentValue) -> Json {
    let group = |head: &str, inner: &[ComponentValue]| {
        let mut a = vec![s(head)];
        a.extend(inner.iter().map(value));
        Json::Arr(a)
    };
    match v {
        ComponentValue::Token(t) => token(t),
        ComponentValue::Block { kind: k, values } => group(
            match k {
                BlockKind::Paren => "()",
                BlockKind::Square => "[]",
                BlockKind::Curly => "{}",
            },
            values,
        ),
        ComponentValue::Function { name, values } => group(name, values).with_head("function"),
    }
}

trait WithHead {
    fn with_head(self, head: &str) -> Json;
}
impl WithHead for Json {
    /// `["name", ...]` becomes `["function", "name", ...]`.
    fn with_head(self, head: &str) -> Json {
        match self {
            Json::Arr(mut a) => {
                a.insert(0, s(head));
                Json::Arr(a)
            }
            o => o,
        }
    }
}

/// Drops number reprs and the positional `eof-in-*` error entries from an
/// expected value (returning how many were dropped, by name).
fn clean(j: &Json, errs: &mut BTreeMap<String, usize>) -> Option<Json> {
    match j {
        Json::Arr(a) => {
            let head = a.first().and_then(Json::as_str);
            if head == Some("error") {
                let name = a.get(1).and_then(Json::as_str).unwrap_or("?");
                if name.starts_with("eof-in-") {
                    *errs.entry(name.to_string()).or_default() += 1;
                    return None;
                }
                return Some(j.clone());
            }
            let mut out = Vec::new();
            for (i, x) in a.iter().enumerate() {
                if i == 1 && matches!(head, Some("number" | "percentage" | "dimension")) {
                    continue; // repr string
                }
                if i == 3 && head == Some("percentage") {
                    continue; // the percentage token keeps no integer/number type
                }
                if let Some(c) = clean(x, errs) {
                    out.push(c);
                }
            }
            Some(Json::Arr(out))
        }
        _ => Some(j.clone()),
    }
}

/// Why a vector cannot be compared, if so.
fn skip_reason(j: &Json) -> Option<&'static str> {
    match j {
        Json::Str(x) if matches!(x.as_str(), "~=" | "|=" | "^=" | "$=" | "*=" | "||") => {
            Some("match tokens (removed from the spec)")
        }
        Json::Arr(a) => {
            if let (Some("unicode-range"), 1..) = (a.first().and_then(Json::as_str), a.len()) {
                return Some("unicode-range token (removed from the spec)");
            }
            if a.first().and_then(Json::as_str) == Some("error")
                && matches!(a.get(1).and_then(Json::as_str), Some(")" | "]" | "}"))
            {
                return Some("mismatched-closer error entries");
            }
            a.iter().find_map(skip_reason)
        }
        _ => None,
    }
}

#[derive(Default)]
struct Tally {
    run: usize,
    pass: usize,
    skipped: BTreeMap<&'static str, usize>,
}

fn main() {
    let mut dir = None;
    let mut verbose = false;
    let mut only: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-v" => verbose = true,
            "--file" => only = args.next(),
            _ => dir = Some(PathBuf::from(a)),
        }
    }
    let Some(dir) = dir else {
        eprintln!("usage: css-runner <dir-with-json> [-v] [--file NAME]");
        std::process::exit(2);
    };
    std::panic::set_hook(Box::new(|_| {}));
    let mut tally = Tally::default();
    for name in ["component_value_list.json"] {
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let path = dir.join(name);
        let src = match fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("cannot read {}: {e}", path.display());
                std::process::exit(2);
            }
        };
        let doc = match (JsonParser { b: &src, i: 0 }).value() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{name}: {e}");
                std::process::exit(2);
            }
        };
        let items = doc.as_arr().unwrap_or(&[]);
        for (n, pair) in items.chunks(2).enumerate() {
            let [input, expected] = pair else { continue };
            let (Some(input), Some(expected)) = (input.as_str(), expected.as_arr()) else {
                continue;
            };
            if let Some(why) = expected.iter().find_map(skip_reason) {
                *tally.skipped.entry(why).or_default() += 1;
                continue;
            }
            tally.run += 1;
            let mut want_errs = BTreeMap::new();
            let want: Vec<Json> = expected
                .iter()
                .filter_map(|e| clean(e, &mut want_errs))
                .collect();
            let got = catch_unwind(AssertUnwindSafe(|| {
                let p = parse_component_value_list(input);
                let mut errs: BTreeMap<String, usize> = BTreeMap::new();
                for e in &p.errors {
                    if e.starts_with("eof-in-string") || e.starts_with("eof-in-url") {
                        *errs.entry(e.to_string()).or_default() += 1;
                    }
                }
                let vals: Vec<Json> = p.values.iter().map(value).collect();
                let vals: Vec<Json> = vals
                    .iter()
                    .filter_map(|v| clean(v, &mut BTreeMap::new()))
                    .collect();
                (vals, errs)
            }));
            let ok = matches!(&got, Ok((v, e)) if *v == want && *e == want_errs);
            if ok {
                tally.pass += 1;
            } else if verbose {
                println!("FAIL {name} #{n}: {input:?}");
                println!("  want {want:?} {want_errs:?}");
                match got {
                    Ok((v, e)) => println!("  got  {v:?} {e:?}"),
                    Err(_) => println!("  got  PANIC"),
                }
            }
        }
        println!("{name}: run {}, pass {}", tally.run, tally.pass);
    }
    for (why, n) in &tally.skipped {
        println!("skipped {n}: {why}");
    }
    let fail = tally.run - tally.pass;
    println!(
        "TOTAL run {}: pass {}, fail {}; skipped {}",
        tally.run,
        tally.pass,
        fail,
        tally.skipped.values().sum::<usize>()
    );
    if tally.run > 0 {
        println!(
            "pass rate {:.1}%",
            100.0 * tally.pass as f64 / tally.run as f64
        );
    }
}
