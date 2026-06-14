//! PyO3 wrapper over the `safe-json-repair` core. Like the wasm binding, this is
//! a thin marshalling layer — all repair logic lives in the core crate; here we
//! only translate options in and the result out.
//!
//! The exported `repair(input, **options)` returns a `RepairResult` whose shape
//! mirrors the Rust `RepairResult` and the TS one-to-one: `ok`, `value`, `json`,
//! `changed`, `strategy` — with `strategy` as a kebab-case string identical to
//! the corpus annotations and the Rust ladder.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use safe_json_repair::{repair as core_repair, Fallback, Options, Strategy};

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

/// The result of a repair attempt. Always returned — `repair` never raises for
/// input shape (only for invalid *options*).
#[pyclass(frozen, module = "safe_json_repair", name = "RepairResult")]
struct PyRepairResult {
    /// Whether a meaningful value was recovered (`False` only for fallback).
    #[pyo3(get)]
    ok: bool,
    /// The recovered value as a native Python object (the fallback value when
    /// `ok` is `False`) — what `json.loads` would have produced.
    #[pyo3(get)]
    value: Py<PyAny>,
    /// Canonical JSON serialization of `value`.
    #[pyo3(get)]
    json: String,
    /// Whether the original text was altered to produce the result.
    #[pyo3(get)]
    changed: bool,
    /// Which ladder rung produced the result (kebab-case).
    #[pyo3(get)]
    strategy: String,
}

#[pymethods]
impl PyRepairResult {
    fn __repr__(&self) -> String {
        format!(
            "RepairResult(ok={}, strategy={:?}, changed={}, json={:?})",
            if self.ok { "True" } else { "False" },
            self.strategy,
            if self.changed { "True" } else { "False" },
            self.json,
        )
    }
}

fn build_options(
    max_len: Option<usize>,
    fallback: Option<&str>,
    strip_code_fences: Option<bool>,
    unwrap_double_encoded: Option<bool>,
) -> PyResult<Options> {
    let mut opts = Options::default();
    if let Some(n) = max_len {
        opts.max_len = n;
    }
    if let Some(f) = fallback {
        // Fail loudly on a typo'd option rather than silently picking a default.
        opts.fallback = match f {
            "null" => Fallback::Null,
            "empty-object" => Fallback::EmptyObject,
            other => {
                return Err(PyValueError::new_err(format!(
                    "invalid fallback {other:?}: expected 'null' or 'empty-object'"
                )));
            }
        };
    }
    if let Some(b) = strip_code_fences {
        opts.strip_code_fences = b;
    }
    if let Some(b) = unwrap_double_encoded {
        opts.unwrap_double_encoded = b;
    }
    Ok(opts)
}

/// Repair `input` into a valid JSON value. Never raises on input shape — any
/// input, including pure garbage, returns a deterministic `RepairResult`.
#[pyfunction]
#[pyo3(signature = (input, *, max_len=None, fallback=None, strip_code_fences=None, unwrap_double_encoded=None))]
fn repair(
    py: Python<'_>,
    input: &str,
    max_len: Option<usize>,
    fallback: Option<&str>,
    strip_code_fences: Option<bool>,
    unwrap_double_encoded: Option<bool>,
) -> PyResult<PyRepairResult> {
    let opts = build_options(max_len, fallback, strip_code_fences, unwrap_double_encoded)?;
    let r = core_repair(input, &opts);
    // serde_json::Value → native Python (dict/list/str/int/float/bool/None).
    // JSON null maps to Python None, matching `json.loads`.
    let value = pythonize::pythonize(py, &r.value)
        .map_err(|e| PyValueError::new_err(e.to_string()))?
        .unbind();
    Ok(PyRepairResult {
        ok: r.ok,
        value,
        json: r.json,
        changed: r.changed,
        strategy: strategy_name(r.strategy).to_string(),
    })
}

#[pymodule]
fn _safe_json_repair(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRepairResult>()?;
    m.add_function(wrap_pyfunction!(repair, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
