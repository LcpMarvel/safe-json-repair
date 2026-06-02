#![no_main]
//! libFuzzer target for `safe_json_repair::repair`.
//!
//! Run with: `cargo +nightly fuzz run repair` (requires `cargo install cargo-fuzz`).
//!
//! Invariants asserted on every input — these are the PRD's robustness bar:
//!   * never panics / never loops forever (the harness enforces wall-clock),
//!   * the result is always valid, re-parseable JSON,
//!   * a successful repair of valid JSON equals what `serde_json` parses.

use libfuzzer_sys::fuzz_target;
use safe_json_repair::{repair, Options, Strategy};
use serde_json::Value;

/// Structural equality that tolerates `serde_json`'s float round-trip noise.
///
/// `serde_json`'s float parser is not correctly-rounded, so a
/// parse → serialize → parse cycle can drift by ~1 ULP (e.g. `3e50` and
/// `123456789.123456789` never reach a serialization fixed point). That drift
/// is a `serde_json` property, not something `repair` controls or promises.
///
/// The tolerance is scoped as tightly as the noise it absorbs:
///   * **Integers** (`i64`/`u64`-backed) are compared *exactly* — they are not
///     subject to float round-trip drift, so any difference is a real bug.
///   * **Floats** get a *few-ULP* window (`8 * f64::EPSILON`, relative) — wide
///     enough for serde's 1-ULP drift, ~4000× tighter than the old `1e-12`, so
///     genuine numeric corruption above a handful of ULPs is still caught.
fn value_eq_modulo_float_noise(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if x.is_f64() || y.is_f64() {
                match (x.as_f64(), y.as_f64()) {
                    (Some(fx), Some(fy)) => {
                        fx == fy
                            || (fx - fy).abs() <= fx.abs().max(fy.abs()) * 8.0 * f64::EPSILON
                    }
                    // A float-backed Number whose `as_f64` fails is degenerate;
                    // fall back to exact comparison rather than pretend equal.
                    _ => x == y,
                }
            } else {
                // Both integer-backed: exact, no tolerance.
                x == y
            }
        }
        (Value::Array(xs), Value::Array(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys)
                    .all(|(p, q)| value_eq_modulo_float_noise(p, q))
        }
        (Value::Object(xs), Value::Object(ys)) => {
            xs.len() == ys.len()
                && xs.iter().all(|(k, v)| {
                    ys.get(k)
                        .is_some_and(|w| value_eq_modulo_float_noise(v, w))
                })
        }
        _ => a == b,
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let result = repair(input, &Options::default());

    // The emitted JSON must always parse back.
    let reparsed = serde_json::from_str::<Value>(&result.json)
        .expect("result.json must be valid JSON");
    assert!(
        value_eq_modulo_float_noise(&reparsed, &result.value),
        "result.json must serialize result.value (input {input:?})"
    );

    // Fast-path fidelity: if the input was already valid JSON, we must not
    // alter its parsed value.
    if let Ok(expected) = serde_json::from_str::<serde_json::Value>(input) {
        // A bare JSON string encoding an object/array is intentionally
        // unwrapped (UnwrapDouble); everything else must be byte-faithful.
        if result.strategy == Strategy::Parse {
            assert_eq!(result.value, expected, "fast-path must not rewrite valid JSON");
            assert!(!result.changed, "fast-path must not mark changed");
        }
    }
});
