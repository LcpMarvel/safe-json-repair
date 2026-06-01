//! Coverage for the non-default `Options` knobs (the corpus runs defaults).

use safe_json_repair::{repair, Fallback, Options, Strategy};
use serde_json::{json, Value};

#[test]
fn fallback_empty_object() {
    let opts = Options { fallback: Fallback::EmptyObject, ..Options::default() };
    let r = repair("not json at all", &opts);
    assert!(!r.ok);
    assert_eq!(r.strategy, Strategy::Fallback);
    assert_eq!(r.value, json!({}));
    assert_eq!(r.json, "{}");
}

#[test]
fn fallback_null_default() {
    let r = repair("not json at all", &Options::default());
    assert!(!r.ok);
    assert_eq!(r.value, Value::Null);
}

#[test]
fn disable_code_fences() {
    let opts = Options { strip_code_fences: false, ..Options::default() };
    // With fences disabled, level 1 is skipped; the tolerant parser still sees
    // the backticks as garbage but recovers the embedded object.
    let r = repair("```json{\"a\":1}```", &opts);
    // Either way it must not claim StripFences.
    assert_ne!(r.strategy, Strategy::StripFences);
}

#[test]
fn disable_double_unwrap_keeps_string() {
    let opts = Options { unwrap_double_encoded: false, ..Options::default() };
    let r = repair(r#""{\"a\":1}""#, &opts);
    assert_eq!(r.strategy, Strategy::Parse);
    assert_eq!(r.value, Value::String(r#"{"a":1}"#.to_string()));
}

#[test]
fn oversize_input_falls_back_without_work() {
    let opts = Options { max_len: 8, ..Options::default() };
    // A perfectly valid JSON, but longer than max_len → deterministic fallback.
    let r = repair(r#"{"a":12345}"#, &opts);
    assert!(!r.ok);
    assert_eq!(r.strategy, Strategy::Fallback);
}

#[test]
fn changed_flag_semantics() {
    assert!(!repair(r#"{"a":1}"#, &Options::default()).changed); // clean parse
    assert!(repair(r#"{"a":1,}"#, &Options::default()).changed); // trailing comma
    assert!(repair(r#"{"a":1]}"#, &Options::default()).changed); // tolerant
}
