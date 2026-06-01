//! String-level cleanup passes for repair levels 1–4.
//!
//! These are deliberately conservative: each pass either returns the input
//! unchanged or makes one well-defined structural edit. The heavy lifting for
//! genuinely broken structure lives in [`crate::tolerant`].

/// Level 1 — strip a Markdown code fence wrapper.
///
/// Handles both the multi-line form (a ```` ```json ```` line, the JSON body,
/// then a closing ```` ``` ```` line) and the inline form where the fence,
/// language tag, body, and closing fence all sit on one line. Returns `None`
/// when the input is not fenced, so the caller can tell "fence removed" from
/// "nothing to do".
pub fn strip_code_fences(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let inner = trimmed.strip_prefix("```")?;
    // Drop an optional language tag that immediately follows the opening fence
    // (e.g. `json`, `json5`). It runs until a newline or the first non
    // identifier byte, whichever comes first.
    let inner = inner.trim_start_matches(|c: char| c.is_ascii_alphanumeric());
    // Drop the closing fence if present.
    let inner = inner.strip_suffix("```").unwrap_or(inner);
    let inner = inner.trim();
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

/// Level 2 — tame literal control characters that appear *inside* string
/// values.
///
/// Per PRD decision D5 we **preserve** the common whitespace controls by
/// turning the literal byte into its escape sequence (a literal newline byte
/// becomes the two characters `\` `n`), so multi-line string values survive.
/// Every other `0x00..=0x1F` byte is dropped, matching CodeWhale.
///
/// Control characters outside of strings are left untouched — they are
/// insignificant JSON whitespace or will be handled by the tolerant parser.
pub fn escape_control_chars_in_strings(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escaped = false;
    for ch in input.chars() {
        if in_string {
            if escaped {
                out.push(ch);
                escaped = false;
                continue;
            }
            match ch {
                '\\' => {
                    escaped = true;
                    out.push(ch);
                }
                '"' => {
                    in_string = false;
                    out.push(ch);
                }
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                c if (c as u32) < 0x20 => { /* drop other control chars */ }
                c => out.push(c),
            }
        } else {
            if ch == '"' {
                in_string = true;
            }
            out.push(ch);
        }
    }
    out
}

/// Level 3 — drop trailing commas before a closing `}` or `]`.
///
/// String-aware: a `,}` sequence inside a string value is left alone. Also
/// drops a comma that is the final non-whitespace character.
pub fn strip_trailing_commas(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut in_string = false;
    let mut escaped = false;
    for (i, &ch) in chars.iter().enumerate() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            out.push(ch);
            continue;
        }
        if ch == ',' {
            // Look ahead past whitespace for a closer or end-of-input.
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j >= chars.len() || chars[j] == '}' || chars[j] == ']' {
                // Skip this comma.
                continue;
            }
        }
        out.push(ch);
    }
    out.into_iter().collect()
}
