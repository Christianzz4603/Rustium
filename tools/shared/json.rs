//! Minimal JSON reader shared by the vector runners (included with `#[path]`).
#![allow(dead_code)]

/// Stands in for a lone surrogate, which a Rust `String` cannot hold.
pub const LONE_SURROGATE: char = '\u{10FFFF}';

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(v) => v.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
}

pub struct JsonParser<'a> {
    pub b: &'a [u8],
    pub i: usize,
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
    pub fn value(&mut self) -> Result<Json, String> {
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
pub fn unescape_double(s: &str) -> String {
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
