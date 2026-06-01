//! The golden corpus is the executable, living PRD. Each case asserts the
//! input recovers to a valid JSON value, equals the annotated `expect` object
//! key-for-key, and hits the expected strategy. Loaded from the shared
//! `corpus/cases.json` so the JS/WASM binding can run the very same cases.

use safe_json_repair::{repair, Options, Strategy};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Case {
    name: String,
    #[allow(dead_code)]
    desc: String,
    input: String,
    expect: Value,
    strategy: String,
}

fn strategy_name(s: Strategy) -> &'static str {
    match s {
        Strategy::Parse => "parse",
        Strategy::StripFences => "strip-fences",
        Strategy::StripControls => "strip-controls",
        Strategy::StripTrailingCommas => "strip-trailing-commas",
        Strategy::UnwrapDouble => "unwrap-double",
        Strategy::Tolerant => "tolerant",
        Strategy::Fallback => "fallback",
    }
}

fn load_cases() -> Vec<Case> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/cases.json");
    let raw = std::fs::read_to_string(path).expect("read corpus/cases.json");
    serde_json::from_str(&raw).expect("parse corpus/cases.json")
}

#[test]
fn golden_corpus() {
    let opts = Options::default();
    let mut failures = Vec::new();

    for case in load_cases() {
        let result = repair(&case.input, &opts);

        // 1. Output is always valid JSON (round-trips through serde).
        if serde_json::from_str::<Value>(&result.json).is_err() {
            failures.push(format!("{}: result.json is not valid JSON: {}", case.name, result.json));
            continue;
        }

        // 2. Strategy check (skip when annotated "any").
        if case.strategy != "any" {
            let got = strategy_name(result.strategy);
            if got != case.strategy {
                failures.push(format!(
                    "{}: strategy expected {} got {}",
                    case.name, case.strategy, got
                ));
            }
        }

        // 3. Value check. For fallback/any we only assert the contract, not a
        //    specific value (garbage / pathological inputs).
        match case.strategy.as_str() {
            "fallback" => {
                if result.ok {
                    failures.push(format!("{}: fallback case but ok=true", case.name));
                }
                if result.value != Value::Null {
                    failures.push(format!("{}: fallback expected null, got {}", case.name, result.value));
                }
            }
            "any" => {
                if !result.ok {
                    failures.push(format!("{}: expected a recovered value, got fallback", case.name));
                }
            }
            _ => {
                if result.value != case.expect {
                    failures.push(format!(
                        "{}: value mismatch\n  expected: {}\n  got:      {}",
                        case.name, case.expect, result.value
                    ));
                }
            }
        }
    }

    assert!(failures.is_empty(), "corpus failures:\n{}", failures.join("\n"));
}

/// The v1 acceptance red-line: the real-world broken sample must recover the
/// full graph (4 lanes / 11 edges) **and keep the sibling `summary`** — the
/// exact field every existing library drops.
#[test]
fn real_sample_keeps_summary() {
    let case = load_cases()
        .into_iter()
        .find(|c| c.name == "C1-full")
        .expect("C1-full present");
    let r = repair(&case.input, &Options::default());
    assert!(r.ok);
    assert_eq!(r.strategy, Strategy::Tolerant);
    let obj = r.value.as_object().expect("object");
    assert_eq!(obj["lanes"].as_array().unwrap().len(), 4, "all 4 lanes");
    assert_eq!(obj["edges"].as_array().unwrap().len(), 11, "all 11 edges");
    assert_eq!(
        obj["summary"], case.expect["summary"],
        "summary must survive the premature root close"
    );
}
