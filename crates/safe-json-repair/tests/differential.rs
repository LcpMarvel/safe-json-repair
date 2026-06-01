//! Differential test: for valid JSON the library must behave like a fast-path —
//! it parses to exactly what `serde_json` parses, reports `Strategy::Parse`,
//! and never marks the input as changed. This guards the PRD's "合法输入零改写"
//! invariant (corpus C10) across a broad sample, not just one case.

use safe_json_repair::{repair, Options, Strategy};
use serde_json::Value;

const VALID: &[&str] = &[
    r#"{}"#,
    r#"[]"#,
    r#"null"#,
    r#"true"#,
    r#"false"#,
    r#"0"#,
    r#"-1"#,
    r#"3.14"#,
    r#"1e10"#,
    r#"-2.5e-3"#,
    r#""hello""#,
    r#""with \"escapes\" and \n newlines""#,
    r#""unicode é 😀""#,
    r#"{"a":1,"b":2,"c":3}"#,
    r#"{"a":[1,2,3],"b":"x"}"#,
    r#"[{"x":1},{"y":2},{"z":3}]"#,
    r#"{"nested":{"deep":{"deeper":{"v":true}}}}"#,
    r#"{"o":{"a":1}, "b":2}"#,
    r#"{"items":[{"x":1},{"y":2}],"n":"t"}"#,
    r#"{"mixed":[1,"two",true,null,{"k":"v"},[9]]}"#,
    r#"{"empty_obj":{},"empty_arr":[],"empty_str":""}"#,
    r#"{"big":123456789012345,"neg":-987654321}"#,
    r#"   {"leading":"ws"}   "#,
    r#"{"unicode_key_é":"value"}"#,
];

#[test]
fn valid_json_is_fast_path() {
    for &input in VALID {
        let r = repair(input, &Options::default());
        let expected: Value = serde_json::from_str(input).expect("test input is valid JSON");

        assert!(r.ok, "{input:?}: expected ok");
        assert_eq!(r.strategy, Strategy::Parse, "{input:?}: expected fast-path Parse");
        assert!(!r.changed, "{input:?}: fast-path must not mark changed");
        assert_eq!(r.value, expected, "{input:?}: value must equal serde_json");

        // r.json must round-trip to the same value as the input.
        let from_json: Value = serde_json::from_str(&r.json).expect("r.json valid");
        assert_eq!(from_json, expected, "{input:?}: r.json must round-trip");
    }
}

/// A valid JSON *string* that does not encode an object/array stays a string
/// (no spurious double-unwrap). A string that *does* encode one is unwrapped.
#[test]
fn unwrap_only_for_object_or_array() {
    let opts = Options::default();

    // Bare string -> stays a string (Parse), not unwrapped.
    let r = repair(r#""just text""#, &opts);
    assert_eq!(r.strategy, Strategy::Parse);
    assert_eq!(r.value, Value::String("just text".into()));

    // String encoding a number -> NOT unwrapped (D3: object/array only).
    let r = repair(r#""42""#, &opts);
    assert_eq!(r.strategy, Strategy::Parse);
    assert_eq!(r.value, Value::String("42".into()));

    // String encoding an object -> unwrapped.
    let r = repair(r#""{\"a\":1}""#, &opts);
    assert_eq!(r.strategy, Strategy::UnwrapDouble);
    assert_eq!(r.value, serde_json::json!({"a":1}));
}
