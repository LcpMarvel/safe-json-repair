//! `safe-json-repair` — a JSON repair library that **never throws** and
//! **never silently drops data**.
//!
//! It targets the way LLMs break JSON in tool-calling / structured-output:
//! a stray closing delimiter that prematurely closes the parent or root
//! object, orphaning the sibling keys that follow. Every existing repair
//! library either throws on this shape or silently drops those siblings.
//! Our stack-aware tolerant parser ([`tolerant`]) keeps them.
//!
//! ```
//! use safe_json_repair::{repair, Options, Strategy};
//!
//! // Premature root close + sibling key (the real-world DeepSeek shape).
//! let r = repair(r#"{"a":1}, "b":2}"#, &Options::default());
//! assert!(r.ok);
//! assert_eq!(r.strategy, Strategy::Tolerant);
//! assert_eq!(r.json, r#"{"a":1,"b":2}"#);
//! ```
//!
//! The public entry point [`repair`] runs a fixed ladder of strategies and
//! reports which one produced the result, whether the input was changed, and a
//! canonical JSON serialization — so callers can log, alert, and meter.

mod preprocess;
pub mod tolerant;

use serde_json::{Map, Value};

/// What to return when every repair strategy fails (corpus C11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fallback {
    /// Return `null`. The general-purpose default — callers check for null.
    #[default]
    Null,
    /// Return `{}`. For "must keep running" consumers like Tramito's backend.
    EmptyObject,
}

/// Tuning knobs for [`repair`]. [`Options::default`] matches the PRD defaults.
#[derive(Debug, Clone)]
pub struct Options {
    /// Inputs longer than this (in bytes) skip straight to the fallback,
    /// guarding against pathological inputs. Default 5_000_000.
    pub max_len: usize,
    /// What to return when nothing parses. Default [`Fallback::Null`].
    pub fallback: Fallback,
    /// Strip Markdown code fences (level 1). Default `true`.
    pub strip_code_fences: bool,
    /// Unwrap a JSON string that itself encodes an object/array (level 4).
    /// Default `true`.
    pub unwrap_double_encoded: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            max_len: 5_000_000,
            fallback: Fallback::Null,
            strip_code_fences: true,
            unwrap_double_encoded: true,
        }
    }
}

/// Which rung of the repair ladder produced the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Level 0 — strict parse of valid JSON, returned verbatim.
    Parse,
    /// Level 1 — Markdown code fence stripped.
    StripFences,
    /// Level 2 — control characters inside strings tamed.
    StripControls,
    /// Level 3 — trailing commas removed.
    StripTrailingCommas,
    /// Level 4 — double-encoded JSON string unwrapped.
    UnwrapDouble,
    /// Level 5 — stack-aware tolerant parser (the differentiator).
    Tolerant,
    /// Level 6 — everything failed; configured fallback returned.
    Fallback,
}

/// The result of a repair attempt. Always returned — [`repair`] never panics.
#[derive(Debug, Clone)]
pub struct RepairResult {
    /// Whether a meaningful value was recovered (`false` only for fallback).
    pub ok: bool,
    /// The recovered value (the fallback value when `ok` is false).
    pub value: Value,
    /// Canonical JSON serialization of [`value`](Self::value).
    pub json: String,
    /// Whether the original text was altered to produce the result.
    pub changed: bool,
    /// Which ladder rung produced the result.
    pub strategy: Strategy,
}

impl RepairResult {
    fn new(value: Value, changed: bool, strategy: Strategy, ok: bool) -> Self {
        let json = serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string());
        RepairResult {
            ok,
            value,
            json,
            changed,
            strategy,
        }
    }
}

/// Repair `input` into a valid JSON value. Never panics, never throws.
///
/// Runs levels 0–6 in order and returns at the first rung that yields a
/// meaningful result. See [`Strategy`] for the ladder.
pub fn repair(input: &str, opts: &Options) -> RepairResult {
    let fallback_value = || match opts.fallback {
        Fallback::Null => Value::Null,
        Fallback::EmptyObject => Value::Object(Map::new()),
    };

    // Oversized input: refuse to work, return fallback deterministically.
    if input.len() > opts.max_len {
        return RepairResult::new(fallback_value(), true, Strategy::Fallback, false);
    }

    // Level 0 — strict parse. Valid JSON is returned verbatim (no rewrite).
    if let Ok(value) = serde_json::from_str::<Value>(input) {
        // A valid JSON *string* that itself encodes an object/array is the
        // double-encoded case (level 4) — unwrap it here so we don't report a
        // bare string as a clean parse.
        if opts.unwrap_double_encoded {
            if let Value::String(inner) = &value {
                if let Some(unwrapped) = try_unwrap_double(inner) {
                    return RepairResult::new(unwrapped, true, Strategy::UnwrapDouble, true);
                }
            }
        }
        return RepairResult::new(value, false, Strategy::Parse, true);
    }

    // `work` carries cumulative cleanup forward so later levels compose with
    // earlier ones (e.g. fenced + trailing comma).
    let mut work = input.to_string();

    // Level 1 — strip code fences.
    if opts.strip_code_fences {
        if let Some(stripped) = preprocess::strip_code_fences(&work) {
            if let Ok(value) = serde_json::from_str::<Value>(&stripped) {
                return RepairResult::new(value, true, Strategy::StripFences, true);
            }
            work = stripped;
        }
    }

    // Level 2 — tame control characters inside strings.
    let controls = preprocess::escape_control_chars_in_strings(&work);
    if controls != work {
        if let Ok(value) = serde_json::from_str::<Value>(&controls) {
            return RepairResult::new(value, true, Strategy::StripControls, true);
        }
        work = controls;
    }

    // Level 3 — strip trailing commas.
    let no_commas = preprocess::strip_trailing_commas(&work);
    if no_commas != work {
        if let Ok(value) = serde_json::from_str::<Value>(&no_commas) {
            return RepairResult::new(value, true, Strategy::StripTrailingCommas, true);
        }
        work = no_commas;
    }

    // Level 4 — unwrap a double-encoded JSON string.
    if opts.unwrap_double_encoded {
        if let Ok(Value::String(inner)) = serde_json::from_str::<Value>(&work) {
            if let Some(unwrapped) = try_unwrap_double(&inner) {
                return RepairResult::new(unwrapped, true, Strategy::UnwrapDouble, true);
            }
        }
    }

    // Level 5 — stack-aware tolerant parse (the soul of the library).
    let value = tolerant::parse(&work);
    if is_meaningful(&value, &work) {
        return RepairResult::new(value, true, Strategy::Tolerant, true);
    }

    // Level 6 — fallback. We never throw.
    RepairResult::new(fallback_value(), true, Strategy::Fallback, false)
}

/// Try to unwrap a string whose contents are themselves a JSON object/array.
/// Per PRD D3 we only unwrap to object/array, never to a scalar, to avoid
/// mangling a string field that merely happens to be JSON-ish.
fn try_unwrap_double(inner: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(inner) {
        Ok(v @ Value::Object(_)) | Ok(v @ Value::Array(_)) => Some(v),
        _ => None,
    }
}

/// Did the tolerant parser actually recover something? A bare `null` only
/// counts when the input literally was `null`; otherwise it means "garbage in,
/// nothing out" and we should fall through to the configured fallback.
fn is_meaningful(value: &Value, src: &str) -> bool {
    match value {
        Value::Null => src.trim() == "null",
        _ => true,
    }
}
