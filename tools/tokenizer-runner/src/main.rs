//! Runs html5lib-format tokenizer `.test` vectors against `rustium-html`.
//!
//! ```text
//! cargo run -p tokenizer-runner -- <dir-with-.test-files> [-v] [--file NAME]
//! ```
//!
//! The vectors come from <https://github.com/html5lib/html5lib-tests> (`tokenizer/`)
//! and are not vendored here. Use the debug profile (release is `panic = "abort"`).
//!
//! Reporting rules (nothing is hidden):
//! - A test the tokenizer output differs on, or that panics, is a **failure**.
//! - A test the runner cannot execute faithfully is **skipped** and counted:
//!   the `CDATA section state` initial state (needs foreign content, which the
//!   tree builder decides), and inputs containing lone UTF-16 surrogates (they
//!   cannot be Rust strings).
//! - Parse-error lists (`errors`) are not compared; only tokens are.
//! - `xmlViolationTests` (infoset coercion) are not run.
//! - Consecutive character tokens are merged, as the vectors expect.

use std::collections::BTreeMap;
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use rustium_html::{RawKind, Token, Tokenizer};

/// Stands in for a lone surrogate, which a Rust `String` cannot hold.
const LONE_SURROGATE: char = '\u{10FFFF}';

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(v) => v.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
}

struct JsonParser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> JsonParser<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("JSON error at byte {}: {what}", self.i))
    }
    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.b.get(self.i) {
            None => self.err("unexpected end"),
            Some(b'n') => self.lit("null", Json::Null),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'[') => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.b.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(Json::Arr(v));
                    }
                    if !v.is_empty() {
                        if self.b.get(self.i) != Some(&b',') {
                            return self.err("expected , or ]");
                        }
                        self.i += 1;
                    }
                    v.push(self.value()?);
                }
            }
            Some(b'{') => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.b.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(Json::Obj(v));
                    }
                    if !v.is_empty() {
                        if self.b.get(self.i) != Some(&b',') {
                            return self.err("expected , or }");
                        }
                        self.i += 1;
                        self.ws();
                    }
                    if self.b.get(self.i) != Some(&b'"') {
                        return self.err("expected key");
                    }
                    let k = self.string()?;
                    self.ws();
                    if self.b.get(self.i) != Some(&b':') {
                        return self.err("expected :");
                    }
                    self.i += 1;
                    v.push((k, self.value()?));
                }
            }
            Some(_) => {
                let start = self.i;
                while self.i < self.b.len()
                    && matches!(
                        self.b[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                std::str::from_utf8(&self.b[start..self.i])
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .map(Json::Num)
                    .ok_or_else(|| format!("JSON error at byte {start}: bad value"))
            }
        }
    }
    fn lit(&mut self, word: &str, v: Json) -> Result<Json, String> {
        if self.b[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            self.err("bad literal")
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        let s = self
            .b
            .get(self.i..self.i + 4)
            .and_then(|s| std::str::from_utf8(s).ok());
        let v = s.and_then(|s| u32::from_str_radix(s, 16).ok());
        match v {
            Some(v) => {
                self.i += 4;
                Ok(v)
            }
            None => self.err("bad \\u escape"),
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while self.i < self.b.len() && self.b[self.i] != b'"' && self.b[self.i] != b'\\' {
                self.i += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.b[start..self.i])
                    .map_err(|_| "invalid UTF-8".to_string())?,
            );
            match self.b.get(self.i) {
                None => return self.err("unterminated string"),
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(_) => {
                    self.i += 1;
                    let c = *self.b.get(self.i).ok_or("bad escape")?;
                    self.i += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            out.push(self.combine(hi)?);
                        }
                        _ => return self.err("unknown escape"),
                    }
                }
            }
        }
    }
    /// Turns a UTF-16 unit (already read) into a char, reading a low surrogate if needed.
    fn combine(&mut self, hi: u32) -> Result<char, String> {
        if (0xD800..0xDC00).contains(&hi) && self.b[self.i..].starts_with(b"\\u") {
            let save = self.i;
            self.i += 2;
            let lo = self.hex4()?;
            if (0xDC00..0xE000).contains(&lo) {
                return Ok(
                    char::from_u32(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00))
                        .unwrap_or(LONE_SURROGATE),
                );
            }
            self.i = save;
        }
        Ok(char::from_u32(hi).unwrap_or(LONE_SURROGATE))
    }
}

/// Resolves `\uXXXX` escapes inside a string for tests with `doubleEscaped`.
fn unescape_double(s: &str) -> String {
    let mut p = JsonParser {
        b: s.as_bytes(),
        i: 0,
    };
    let mut out = String::new();
    while p.i < p.b.len() {
        if p.b[p.i] == b'\\' && p.b.get(p.i + 1) == Some(&b'u') {
            p.i += 2;
            match p.hex4() {
                Ok(hi) => out.push(p.combine(hi).unwrap_or(LONE_SURROGATE)),
                Err(_) => out.push('\\'),
            }
        } else {
            let ch = s[p.i..].chars().next().unwrap_or('\u{fffd}');
            out.push(ch);
            p.i += ch.len_utf8();
        }
    }
    out
}

/// A token in the form the vectors use.
#[derive(Debug, Clone, PartialEq)]
enum Expect {
    Doctype(Option<String>, Option<String>, Option<String>, bool),
    Start(String, Vec<(String, String)>, bool),
    End(String),
    Comment(String),
    Chars(String),
}

fn normalize(tokens: Vec<Token>) -> Vec<Expect> {
    let mut out: Vec<Expect> = Vec::new();
    for t in tokens {
        match t {
            Token::Eof => {}
            Token::Character(s) => {
                if let Some(Expect::Chars(prev)) = out.last_mut() {
                    prev.push_str(&s);
                } else {
                    out.push(Expect::Chars(s));
                }
            }
            Token::Comment(c) => out.push(Expect::Comment(c)),
            Token::EndTag { name } => out.push(Expect::End(name)),
            Token::StartTag {
                name,
                mut attrs,
                self_closing,
            } => {
                attrs.sort();
                out.push(Expect::Start(name, attrs, self_closing));
            }
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => out.push(Expect::Doctype(name, public_id, system_id, !force_quirks)),
        }
    }
    out
}

fn opt_str(j: Option<&Json>, dbl: bool) -> Option<String> {
    j.and_then(Json::as_str).map(|s| fix(s, dbl))
}

fn fix(s: &str, dbl: bool) -> String {
    if dbl {
        unescape_double(s)
    } else {
        s.to_string()
    }
}

fn expected(out: &[Json], dbl: bool) -> Result<Vec<Expect>, String> {
    let mut v: Vec<Expect> = Vec::new();
    for t in out {
        let a = t.as_arr().ok_or("output item is not an array")?;
        let kind = a
            .first()
            .and_then(Json::as_str)
            .ok_or("missing token kind")?;
        let s = |i: usize| a.get(i).and_then(Json::as_str).map(|s| fix(s, dbl));
        match kind {
            "Character" => {
                let text = s(1).ok_or("Character without text")?;
                if let Some(Expect::Chars(prev)) = v.last_mut() {
                    prev.push_str(&text);
                } else {
                    v.push(Expect::Chars(text));
                }
            }
            "Comment" => v.push(Expect::Comment(s(1).ok_or("Comment without text")?)),
            "EndTag" => v.push(Expect::End(s(1).ok_or("EndTag without name")?)),
            "StartTag" => {
                let mut attrs: Vec<(String, String)> = match a.get(2) {
                    Some(Json::Obj(o)) => o
                        .iter()
                        .filter_map(|(k, v)| v.as_str().map(|v| (fix(k, dbl), fix(v, dbl))))
                        .collect(),
                    _ => Vec::new(),
                };
                attrs.sort();
                let sc = matches!(a.get(3), Some(Json::Bool(true)));
                v.push(Expect::Start(
                    s(1).ok_or("StartTag without name")?,
                    attrs,
                    sc,
                ));
            }
            "DOCTYPE" => v.push(Expect::Doctype(
                opt_str(a.get(1), dbl),
                opt_str(a.get(2), dbl),
                opt_str(a.get(3), dbl),
                matches!(a.get(4), Some(Json::Bool(true))),
            )),
            other => return Err(format!("unknown token kind {other}")),
        }
    }
    Ok(v)
}

fn has_surrogate(v: &[Expect], input: &str) -> bool {
    let bad = |s: &str| s.contains(LONE_SURROGATE);
    bad(input)
        || v.iter().any(|e| match e {
            Expect::Chars(s) | Expect::Comment(s) | Expect::End(s) => bad(s),
            Expect::Start(n, a, _) => bad(n) || a.iter().any(|(k, v)| bad(k) || bad(v)),
            Expect::Doctype(n, p, s, _) => [n, p, s].iter().any(|o| o.as_deref().is_some_and(bad)),
        })
}

/// Mirrors the input preprocessing the tokenizer expects (CRLF and CR to LF).
fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn run(input: &str, state: &str, last_start: Option<&str>) -> Option<Vec<Expect>> {
    let mut t = Tokenizer::new(input, true);
    match state {
        "Data state" => {}
        "PLAINTEXT state" => t.set_raw(RawKind::Plaintext, "plaintext"),
        "RCDATA state" => t.set_raw(RawKind::RcData, last_start.unwrap_or("")),
        "RAWTEXT state" => t.set_raw(RawKind::RawText, last_start.unwrap_or("")),
        "Script data state" => t.set_raw(RawKind::Script, last_start.unwrap_or("script")),
        _ => return None,
    }
    let mut toks = Vec::new();
    loop {
        let tok = t.next_token().token;
        if tok == Token::Eof {
            break;
        }
        toks.push(tok);
    }
    Some(normalize(toks))
}

#[derive(Default)]
struct Tally {
    pass: usize,
    fail: usize,
    skip: usize,
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
        eprintln!("usage: tokenizer-runner <dir-with-.test-files> [-v] [--file NAME]");
        std::process::exit(2);
    };
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| {
            eprintln!("cannot read {}: {e}", dir.display());
            std::process::exit(2)
        })
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "test"))
        .collect();
    files.sort();
    std::panic::set_hook(Box::new(|_| {}));
    let mut per_file: BTreeMap<String, Tally> = BTreeMap::new();
    for f in files {
        let name = f
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let src = match fs::read(&f) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{}: {e}", f.display());
                continue;
            }
        };
        let doc = match (JsonParser { b: &src, i: 0 }).value() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{name}: {e}");
                continue;
            }
        };
        let tally = per_file.entry(name.clone()).or_default();
        let tests = doc.get("tests").and_then(Json::as_arr).unwrap_or(&[]);
        for (n, t) in tests.iter().enumerate() {
            let dbl = matches!(t.get("doubleEscaped"), Some(Json::Bool(true)));
            let desc = t.get("description").and_then(Json::as_str).unwrap_or("");
            let Some(raw_input) = t.get("input").and_then(Json::as_str) else {
                tally.skip += 1;
                continue;
            };
            let input = normalize_newlines(&fix(raw_input, dbl));
            let last = t.get("lastStartTag").and_then(Json::as_str);
            let Some(out) = t.get("output").and_then(Json::as_arr) else {
                tally.skip += 1;
                continue;
            };
            let want = match expected(out, dbl) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("{name} #{n}: {e}");
                    tally.skip += 1;
                    continue;
                }
            };
            if has_surrogate(&want, &input) {
                tally.skip += 1;
                continue;
            }
            let states: Vec<&str> = match t.get("initialStates").and_then(Json::as_arr) {
                Some(a) => a.iter().filter_map(Json::as_str).collect(),
                None => vec!["Data state"],
            };
            for st in states {
                let got = catch_unwind(AssertUnwindSafe(|| run(&input, st, last)));
                match got {
                    Ok(None) => tally.skip += 1,
                    Ok(Some(g)) if g == want => tally.pass += 1,
                    other => {
                        tally.fail += 1;
                        if verbose {
                            println!("FAIL {name} #{n} [{st}] {desc}\n  input: {input:?}\n  want:  {want:?}");
                            match other {
                                Ok(g) => println!("  got:   {g:?}"),
                                Err(_) => println!("  got:   PANIC"),
                            }
                        }
                    }
                }
            }
        }
    }
    let (mut p, mut fl, mut s) = (0, 0, 0);
    println!("{:<24} {:>6} {:>6} {:>6}", "file", "pass", "fail", "skip");
    for (n, t) in &per_file {
        println!("{n:<24} {:>6} {:>6} {:>6}", t.pass, t.fail, t.skip);
        p += t.pass;
        fl += t.fail;
        s += t.skip;
    }
    let run = p + fl;
    println!("\nTOTAL run {run}: pass {p}, fail {fl}; skipped {s}");
    if run > 0 {
        println!("pass rate {:.1}%", 100.0 * p as f64 / run as f64);
    }
}
