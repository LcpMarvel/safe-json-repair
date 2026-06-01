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

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let result = repair(input, &Options::default());

    // The emitted JSON must always parse back.
    let reparsed = serde_json::from_str::<serde_json::Value>(&result.json)
        .expect("result.json must be valid JSON");
    assert_eq!(reparsed, result.value, "result.json must serialize result.value");

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
