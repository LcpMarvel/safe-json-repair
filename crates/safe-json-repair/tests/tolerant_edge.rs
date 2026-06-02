//! End-to-end edge cases for the tolerant parser that the golden corpus does
//! not reach. The corpus exercises the root-**object** premature-close
//! heuristic (C1/E7/E8) and the stray-closer path (C3/C8/C9); this file pins
//! the *symmetric* halves and a few escape/encoding paths so a regression in
//! one branch can't hide behind the other.

use safe_json_repair::{repair, Options, Strategy};
use serde_json::json;

/// The root-**array** premature-close heuristic — the mirror image of the
/// root-object case the corpus covers. A `]` followed by a `,` cannot be a
/// real sibling at the root, so the array is re-opened and the sibling
/// element is reclaimed rather than dropped.
#[test]
fn root_array_premature_close_reclaims_sibling() {
    let r = repair(r#"[1,2], 3]"#, &Options::default());
    assert!(r.ok);
    assert_eq!(r.strategy, Strategy::Tolerant);
    assert_eq!(r.value, json!([1, 2, 3]));
}

#[test]
fn root_array_premature_close_keeps_object_siblings() {
    let r = repair(r#"[{"x":1},{"y":2}], {"z":3}]"#, &Options::default());
    assert!(r.ok);
    assert_eq!(r.value, json!([{"x": 1}, {"y": 2}, {"z": 3}]));
}

// Note: missing-own-closer (`{"a":[1,2}`) and missing-separator (`{"a" 1}`,
// `[1 2]`) shapes are pinned directly against the parser in
// `tolerant.rs`'s unit tests; the ladder routing to `Tolerant` for broken
// structure is already exercised by the root-array cases above, so they are
// not re-asserted here.

/// A double-encoded **array** string unwraps, mirroring corpus C6 (object).
#[test]
fn double_encoded_array_unwraps() {
    let r = repair(r#""[1,2,3]""#, &Options::default());
    assert_eq!(r.strategy, Strategy::UnwrapDouble);
    assert_eq!(r.value, json!([1, 2, 3]));
}

/// `strip-controls` must *drop* a non-whitespace control char inside a string
/// while keeping the value otherwise intact — the corpus only covers the
/// *preserve* side (C5, a literal newline).
#[test]
fn control_char_dropped_then_parses() {
    let input = "{\"a\":\"x\u{0001}y\"}";
    let r = repair(input, &Options::default());
    assert!(r.ok);
    assert_eq!(r.strategy, Strategy::StripControls);
    assert_eq!(r.value, json!({"a": "xy"}));
}
