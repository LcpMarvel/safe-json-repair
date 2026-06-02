//! Level 5 — the stack-aware tolerant parser. This is the soul of the library.
//!
//! A hand-written recursive-descent parser that never fails: it always returns
//! *some* `serde_json::Value` for any input. Its differentiator over every
//! existing repair library is how it treats closing delimiters, encoded as two
//! rules:
//!
//! 1. **Mismatched closer → ask the ancestors.** When a container meets a
//!    closer that does not match its own kind, it looks up the open-container
//!    stack. If an *ancestor* owns that closer, the current container stops and
//!    leaves the closer for the ancestor to consume (this repairs a *missing*
//!    own closer, e.g. `{"a":[1,2}` — the array yields to the object). If no
//!    ancestor owns it, the closer is stray and is skipped (this repairs an
//!    *extra* closer, e.g. `{"a":1]}` — the `]` is junk).
//!
//! 2. **Root has no siblings.** After the root value is fully parsed, a
//!    following `,` is impossible in valid JSON — it means an earlier closer
//!    prematurely closed the root. We re-open the root container and keep
//!    reading sibling members. *This single heuristic is what saves the
//!    sibling `summary` key that jsonrepair and every other library drops.*
//!
//! The parser is string/escape aware, depth-bounded to prevent stack overflow,
//! and guarantees forward progress so it can never loop forever.

use serde_json::{Map, Number, Value};

/// Hard recursion bound. Inputs nested deeper than this stop recursing and the
/// over-deep subtree is abandoned as `null`, keeping the stack safe on
/// pathological inputs (corpus C12).
///
/// Kept below serde_json's deserialization recursion limit (128) so the value
/// we emit is guaranteed to be re-parseable by `serde_json` / `JSON.parse` —
/// an over-deep repair that can't be parsed back would defeat the point.
const MAX_DEPTH: usize = 120;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Container {
    Object,
    Array,
}

pub struct Parser {
    chars: Vec<char>,
    pos: usize,
    /// Open containers, root-first. The last element is the current container.
    stack: Vec<Container>,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        Parser {
            chars: input.chars().collect(),
            pos: 0,
            stack: Vec::new(),
        }
    }

    /// Parse the whole input into a single value. Never panics.
    pub fn parse(&mut self) -> Value {
        self.skip_ws();
        // Trailing junk after the root value is ignored — the root heuristic in
        // `parse_object`/`parse_array` already reclaimed any spurious sibling
        // content while the container was on the stack.
        self.parse_value()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Index of the next non-whitespace char at or after `from`.
    fn next_nonws_from(&self, from: usize) -> Option<(usize, char)> {
        let mut i = from;
        while let Some(&c) = self.chars.get(i) {
            if c.is_whitespace() {
                i += 1;
            } else {
                return Some((i, c));
            }
        }
        None
    }

    /// Does any *ancestor* (a container below the current top of stack) match
    /// the given closer? Used to decide "yield to ancestor" vs "stray closer".
    fn ancestor_owns(&self, closer: char) -> bool {
        let want = match closer {
            '}' => Container::Object,
            ']' => Container::Array,
            _ => return false,
        };
        // Skip the current top; inspect everything beneath it.
        let upper = self.stack.len().saturating_sub(1);
        self.stack[..upper].contains(&want)
    }

    fn parse_value(&mut self) -> Value {
        self.skip_ws();
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => Value::String(self.parse_string()),
            Some('t') | Some('f') | Some('n') => self.parse_keyword(),
            Some(c) if c == '-' || c == '+' || c.is_ascii_digit() || c == '.' => {
                self.parse_number()
            }
            _ => Value::Null,
        }
    }

    fn parse_object(&mut self) -> Value {
        // Over-deep: abandon this subtree but stay panic-free.
        if self.stack.len() >= MAX_DEPTH {
            self.skip_balanced();
            return Value::Null;
        }
        self.bump(); // consume '{'
        self.stack.push(Container::Object);
        let mut map = Map::new();
        let is_root = self.stack.len() == 1;

        loop {
            self.skip_ws();
            match self.peek() {
                None => break, // truncated — close implicitly
                Some('}') => {
                    if is_root {
                        // Root heuristic: a `}` followed by `,` is a spurious
                        // closer; the root cannot have a sibling, so reclaim it
                        // and keep reading members.
                        if let Some((_, ',')) = self.next_nonws_from(self.pos + 1) {
                            self.bump(); // '}'
                            self.skip_ws();
                            self.bump(); // ','
                            continue;
                        }
                    }
                    self.bump(); // '}' closes us
                    break;
                }
                Some(']') => {
                    // A `]` here does not match an object.
                    if self.ancestor_owns(']') {
                        // Belongs to an ancestor array: yield without consuming.
                        break;
                    }
                    self.bump(); // stray — skip and continue
                    continue;
                }
                Some(',') => {
                    self.bump(); // tolerate leading / doubled commas
                    continue;
                }
                Some('"') => {
                    let key = self.parse_string();
                    self.skip_ws();
                    if self.peek() == Some(':') {
                        self.bump();
                    }
                    // (missing colon is tolerated — parse the value regardless)
                    let value = self.parse_value();
                    map.insert(key, value);
                    self.consume_member_separator();
                }
                Some(_) => {
                    // Unexpected byte where a key was expected. Make forward
                    // progress so we can never loop forever.
                    self.bump();
                }
            }
        }

        self.stack.pop();
        Value::Object(map)
    }

    fn parse_array(&mut self) -> Value {
        if self.stack.len() >= MAX_DEPTH {
            self.skip_balanced();
            return Value::Null;
        }
        self.bump(); // consume '['
        self.stack.push(Container::Array);
        let mut arr: Vec<Value> = Vec::new();
        let is_root = self.stack.len() == 1;

        loop {
            self.skip_ws();
            match self.peek() {
                None => break,
                Some(']') => {
                    if is_root {
                        if let Some((_, ',')) = self.next_nonws_from(self.pos + 1) {
                            self.bump(); // ']'
                            self.skip_ws();
                            self.bump(); // ','
                            continue;
                        }
                    }
                    self.bump();
                    break;
                }
                Some('}') => {
                    if self.ancestor_owns('}') {
                        break; // yield to ancestor object
                    }
                    self.bump(); // stray
                    continue;
                }
                Some(',') => {
                    self.bump();
                    continue;
                }
                Some(_) => {
                    let before = self.pos;
                    let value = self.parse_value();
                    if self.pos == before {
                        // Garbage that is not a valid value start (e.g. `:` or a
                        // stray non-keyword letter). `parse_value` could not
                        // advance; skip one char to guarantee forward progress.
                        self.bump();
                        continue;
                    }
                    arr.push(value);
                    self.consume_member_separator();
                }
            }
        }

        self.stack.pop();
        Value::Array(arr)
    }

    /// After a member value, swallow an optional `,`. We stop *before* a closer
    /// so the container loop can apply the closer rules; this keeps separator
    /// handling and delimiter handling in one place.
    fn consume_member_separator(&mut self) {
        self.skip_ws();
        if self.peek() == Some(',') {
            self.bump();
        }
    }

    fn parse_string(&mut self) -> String {
        self.bump(); // opening quote
        let mut s = String::new();
        while let Some(c) = self.bump() {
            match c {
                '"' => return s, // closed normally
                '\\' => {
                    match self.bump() {
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some('/') => s.push('/'),
                        Some('b') => s.push('\u{0008}'),
                        Some('f') => s.push('\u{000C}'),
                        Some('n') => s.push('\n'),
                        Some('r') => s.push('\r'),
                        Some('t') => s.push('\t'),
                        Some('u') => s.push(self.parse_unicode_escape()),
                        // Unknown escape — keep the char literally.
                        Some(other) => {
                            s.push('\\');
                            s.push(other);
                        }
                        None => {
                            s.push('\\');
                        }
                    }
                }
                other => s.push(other),
            }
        }
        // Truncated string — return what we accumulated.
        s
    }

    /// Parse the four hex digits of a `\uXXXX` escape (the `\u` is already
    /// consumed). Falls back to U+FFFD on malformed input rather than failing.
    fn parse_unicode_escape(&mut self) -> char {
        let mut code: u32 = 0;
        let mut digits = 0;
        while digits < 4 {
            match self.peek().and_then(|c| c.to_digit(16)) {
                Some(d) => {
                    code = code * 16 + d;
                    self.bump();
                    digits += 1;
                }
                None => break,
            }
        }
        // Surrogate pair: a high surrogate followed by `\uXXXX` low surrogate.
        if (0xD800..=0xDBFF).contains(&code)
            && self.peek() == Some('\\')
            && self.peek_at(1) == Some('u')
        {
            self.bump(); // '\'
            self.bump(); // 'u'
            let mut low: u32 = 0;
            let mut d2 = 0;
            while d2 < 4 {
                match self.peek().and_then(|c| c.to_digit(16)) {
                    Some(d) => {
                        low = low * 16 + d;
                        self.bump();
                        d2 += 1;
                    }
                    None => break,
                }
            }
            if (0xDC00..=0xDFFF).contains(&low) {
                let c = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                return char::from_u32(c).unwrap_or('\u{FFFD}');
            }
        }
        char::from_u32(code).unwrap_or('\u{FFFD}')
    }

    fn parse_keyword(&mut self) -> Value {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphabetic() {
                self.pos += 1;
            } else {
                break;
            }
        }
        let word: String = self.chars[start..self.pos].iter().collect();
        match word.as_str() {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            "null" => Value::Null,
            // Unknown bareword (incl. NaN / Infinity, which are not JSON) — we
            // do not invent a value. Per the non-goals, no JSON5 literals.
            _ => Value::Null,
        }
    }

    fn parse_number(&mut self) -> Value {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit()
                || c == '-'
                || c == '+'
                || c == '.'
                || c == 'e'
                || c == 'E'
            {
                self.pos += 1;
            } else {
                break;
            }
        }
        let token: String = self.chars[start..self.pos].iter().collect();
        match serde_json::from_str::<Number>(&token) {
            Ok(n) => Value::Number(n),
            Err(_) => Value::Null, // malformed numeric run — drop it cleanly
        }
    }

    /// Skip a balanced run starting at the current `{`/`[` (or a single token),
    /// used when we hit the depth limit so we don't recurse but still make
    /// progress past the over-deep subtree.
    fn skip_balanced(&mut self) {
        let open = match self.peek() {
            Some(c @ ('{' | '[')) => c,
            _ => {
                self.bump();
                return;
            }
        };
        let close = if open == '{' { '}' } else { ']' };
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        while let Some(c) = self.bump() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_string = false;
                }
                continue;
            }
            match c {
                '"' => in_string = true,
                d if d == open => depth += 1,
                d if d == close => {
                    depth -= 1;
                    if depth == 0 {
                        return;
                    }
                }
                _ => {}
            }
        }
    }
}

/// Convenience entry point: tolerant-parse `input` into a value.
pub fn parse(input: &str) -> Value {
    Parser::new(input).parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- \uXXXX escape handling (parse_unicode_escape) ---------------------

    #[test]
    fn bmp_unicode_escape() {
        // The raw string keeps the backslash literal, so this is the JSON
        // escape `é` -> é (exercises parse_unicode_escape, not the
        // literal-char branch).
        assert_eq!(parse("\"\\u00e9\""), Value::String("\u{e9}".into()));
    }

    #[test]
    fn surrogate_pair_becomes_astral_char() {
        // `😀` -> 😀 (U+1F600), reassembled from the surrogate halves.
        assert_eq!(parse("\"\\uD83D\\uDE00\""), Value::String("\u{1F600}".into()));
    }

    #[test]
    fn lone_high_surrogate_becomes_replacement_char() {
        assert_eq!(parse(r#""\uD800""#), Value::String("\u{FFFD}".into()));
    }

    #[test]
    fn high_surrogate_followed_by_non_low_consumes_both() {
        // Second `\uXXXX` is consumed as the (bad) low half and the pair
        // collapses to a single replacement char.
        assert_eq!(parse(r#""\uD83D\uD83D""#), Value::String("\u{FFFD}".into()));
    }

    #[test]
    fn short_unicode_escape_uses_digits_read() {
        // Three hex digits then end-of-input -> code 0x004.
        assert_eq!(parse(r#""\u004""#), Value::String("\u{4}".into()));
    }

    // --- string escapes ----------------------------------------------------

    #[test]
    fn known_escapes_decode() {
        assert_eq!(
            parse(r#""a\nb\tc\\d\/e""#),
            Value::String("a\nb\tc\\d/e".into())
        );
    }

    #[test]
    fn unknown_escape_kept_literally() {
        // `\x` is not a JSON escape; we keep both chars rather than invent one.
        assert_eq!(parse(r#""a\xb""#), Value::String("a\\xb".into()));
    }

    #[test]
    fn truncated_string_returns_accumulated() {
        assert_eq!(parse(r#""unterm"#), Value::String("unterm".into()));
    }

    // --- numbers -----------------------------------------------------------

    #[test]
    fn number_forms() {
        assert_eq!(parse("123"), json!(123));
        assert_eq!(parse("1.5e3"), json!(1500.0));
        assert_eq!(parse("-0"), json!(-0.0));
    }

    #[test]
    fn malformed_number_run_becomes_null() {
        // A token serde rejects (e.g. multiple dots) drops cleanly to null
        // rather than panicking or being half-parsed.
        assert_eq!(parse("1.2.3"), Value::Null);
    }

    // --- keywords ----------------------------------------------------------

    #[test]
    fn keywords_and_non_json_barewords() {
        assert_eq!(parse("true"), Value::Bool(true));
        assert_eq!(parse("false"), Value::Bool(false));
        assert_eq!(parse("null"), Value::Null);
        // NaN/Infinity are not JSON — we do not invent a value (no JSON5).
        assert_eq!(parse("NaN"), Value::Null);
        assert_eq!(parse("Infinity"), Value::Null);
    }

    // --- structural tolerance ----------------------------------------------

    #[test]
    fn missing_colon_after_key_is_tolerated() {
        assert_eq!(parse(r#"{"a" 1}"#), json!({"a": 1}));
    }

    #[test]
    fn missing_comma_between_array_elements() {
        assert_eq!(parse("[1 2]"), json!([1, 2]));
    }

    #[test]
    fn missing_own_closer_yields_to_ancestor() {
        // The array has no closing `]`; it meets the object's `}`, sees an
        // ancestor object owns it, and yields without consuming. Distinct path
        // from the stray-closer case `{"a":[1,2}]}` (corpus C8).
        assert_eq!(parse(r#"{"a":[1,2}"#), json!({"a": [1, 2]}));
    }
}
