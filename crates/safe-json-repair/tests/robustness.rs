//! Robustness: the library must return a deterministic, valid-JSON result for
//! any input — including adversarial ones — without panicking, looping, or
//! overflowing the stack. This is the in-tree complement to `cargo-fuzz`
//! (see `fuzz/`); it runs on every `cargo test`.

use safe_json_repair::{repair, Options};
use serde_json::Value;

/// Every result's `json` field must itself be parseable as JSON.
fn assert_valid_result(input: &str) {
    let r = repair(input, &Options::default());
    serde_json::from_str::<Value>(&r.json)
        .unwrap_or_else(|_| panic!("result.json not valid for input {input:?}: {}", r.json));
}

#[test]
fn deep_nesting_does_not_overflow() {
    // Far deeper than MAX_DEPTH; must terminate without a stack overflow.
    let deep_open = "{\"a\":".repeat(100_000);
    assert_valid_result(&deep_open);

    let deep_arr = "[".repeat(100_000);
    assert_valid_result(&deep_arr);

    // Balanced-but-huge depth.
    let mut s = "[".repeat(50_000);
    s.push_str(&"]".repeat(50_000));
    assert_valid_result(&s);
}

#[test]
fn long_input_is_bounded() {
    // Within max_len: handled.
    let long = format!("{{\"k\":\"{}\"}}", "x".repeat(500_000));
    assert_valid_result(&long);

    // Over max_len: deterministic fallback, no work attempted.
    let opts = Options { max_len: 1000, ..Options::default() };
    let r = repair(&"x".repeat(2000), &opts);
    assert!(!r.ok);
    assert_eq!(r.value, Value::Null);
}

#[test]
fn pathological_fragments_never_panic() {
    let cases = [
        "",
        " ",
        "\0",
        "\u{FEFF}", // BOM only
        "{",
        "}",
        "[",
        "]",
        ":",
        ",",
        "\"",
        "\"\\",
        "\"\\u",
        "\"\\uXY",
        "\"\\uD800", // lone high surrogate
        "{\"a\"",
        "{\"a\":",
        "{\"a\":,}",
        "{:}",
        "[,,,]",
        "}}}}}}",
        "]]]]]]",
        "{}{}{}{}",
        "[1,2,3",
        "truefalse",
        "NaN",
        "Infinity",
        "-",
        "1.2.3.4",
        "{\"a\":1}garbage trailing",
        "\u{1F600}\u{1F600}\u{1F600}",
        "{\"k\":\"v\nwith\treal\rcontrols\"}",
    ];
    for c in cases {
        assert_valid_result(c);
    }
}

/// Deterministic pseudo-random fuzz (LCG, no external rng). Generates byte
/// soups and mutations of valid JSON; asserts every result is valid JSON.
#[test]
fn deterministic_fuzz() {
    let seed_corpus = [
        r#"{"a":1,"b":[2,3],"c":"text"}"#,
        r#"[{"x":1},{"y":2}]"#,
        r#"{"deep":{"er":{"est":true}}}"#,
        r#"{"s":"with \"quotes\" and \\ slashes"}"#,
    ];
    let mut state: u64 = 0x9E3779B97F4A7C15;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 33) as u32
    };

    for iter in 0..5000 {
        let base = seed_corpus[(next() as usize) % seed_corpus.len()];
        let mut bytes = base.as_bytes().to_vec();

        // Apply a handful of random mutations.
        let mutations = 1 + (next() % 6);
        for _ in 0..mutations {
            if bytes.is_empty() {
                break;
            }
            let op = next() % 4;
            let idx = (next() as usize) % bytes.len();
            match op {
                0 => {
                    bytes.remove(idx); // delete
                }
                1 => {
                    let junk = b"{}[],:\"\\ \n\0".as_ref();
                    bytes.insert(idx, junk[(next() as usize) % junk.len()]); // insert
                }
                2 => {
                    bytes[idx] = (next() % 256) as u8; // flip
                }
                _ => {
                    bytes.truncate(idx); // truncate
                }
            }
        }

        // Bytes may not be valid UTF-8; repair takes &str, so lossily decode —
        // this still exercises the parser on garbage-shaped text.
        let input = String::from_utf8_lossy(&bytes);
        let r = repair(&input, &Options::default());
        assert!(
            serde_json::from_str::<Value>(&r.json).is_ok(),
            "iter {iter}: invalid result.json {:?} for input {input:?}",
            r.json
        );
    }
}
