//! WASM wrapper over the `safe-json-repair` core. This is the single source of
//! truth for the npm package — all repair logic lives in the core crate; this
//! layer only marshals options in and the result out.
//!
//! The exported `repair(input, options)` mirrors the TS `RepairResult` shape:
//! `{ ok, value, json, changed, strategy }`, with `strategy` as a kebab-case
//! string identical to the corpus annotations and the Rust ladder.

use safe_json_repair::{repair as core_repair, Fallback, Options, Strategy};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Options as they arrive from JS (camelCase, every field optional). Anything
/// omitted falls back to the core default.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct JsOptions {
    max_len: Option<f64>,
    fallback: Option<String>,
    strip_code_fences: Option<bool>,
    unwrap_double_encoded: Option<bool>,
}

/// The result, shaped exactly like the TS `RepairResult`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsResult {
    ok: bool,
    value: serde_json::Value,
    json: String,
    changed: bool,
    strategy: &'static str,
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

fn build_options(js: JsOptions) -> Options {
    let mut opts = Options::default();
    if let Some(n) = js.max_len {
        // JS numbers are f64; clamp sanely into usize.
        opts.max_len = if n.is_finite() && n >= 0.0 {
            n as usize
        } else {
            opts.max_len
        };
    }
    if let Some(f) = js.fallback {
        opts.fallback = match f.as_str() {
            "empty-object" => Fallback::EmptyObject,
            _ => Fallback::Null,
        };
    }
    if let Some(b) = js.strip_code_fences {
        opts.strip_code_fences = b;
    }
    if let Some(b) = js.unwrap_double_encoded {
        opts.unwrap_double_encoded = b;
    }
    opts
}

/// Repair `input` into a valid JSON value. Never throws.
///
/// `options` may be `undefined`/`null` (use defaults) or a plain object with any
/// of `maxLen`, `fallback`, `stripCodeFences`, `unwrapDoubleEncoded`.
#[wasm_bindgen]
pub fn repair(input: &str, options: JsValue) -> Result<JsValue, JsValue> {
    let js_opts: JsOptions = if options.is_undefined() || options.is_null() {
        JsOptions::default()
    } else {
        serde_wasm_bindgen::from_value(options)
            .map_err(|e| JsValue::from_str(&format!("invalid options: {e}")))?
    };

    let r = core_repair(input, &build_options(js_opts));
    let out = JsResult {
        ok: r.ok,
        value: r.value,
        json: r.json,
        changed: r.changed,
        strategy: strategy_name(r.strategy),
    };

    // Match what `JSON.parse` would have produced:
    //   - serde_json maps → plain JS objects (not `Map`);
    //   - JSON `null` (serde unit) → JS `null`, NOT `undefined` — critical for
    //     both the fallback value and any nested null inside a recovered value.
    let serializer = serde_wasm_bindgen::Serializer::new()
        .serialize_maps_as_objects(true)
        .serialize_missing_as_null(true);
    out.serialize(&serializer)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
