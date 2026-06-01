# safe-json-repair

> A JSON repair library that **never throws** and **never silently drops data**.
> Rust core + per-language bindings, all sharing one golden corpus.

LLMs in tool-calling / structured-output mode routinely emit *structurally*
broken JSON. The single most common shape — and the one every existing repair
library mishandles — is a **stray closing delimiter that prematurely closes the
parent or root object**, orphaning the sibling keys that follow:

```jsonc
{"a":1}, "b":2}      // a `}` closed the root too early; "b" is now an orphan
```

* [`jsonrepair`](https://github.com/josdejong/jsonrepair) (1.78M/wk) — **throws**.
* `@qraftr/json-repair`, `jaison`, `json-repair-js` — recover the structure but
  **silently drop** the sibling key (`"b"` / a real `summary` field).

`safe-json-repair` keeps it:

```rust
use safe_json_repair::{repair, Options, Strategy};

let r = repair(r#"{"a":1}, "b":2}"#, &Options::default());
assert!(r.ok);
assert_eq!(r.strategy, Strategy::Tolerant);
assert_eq!(r.json, r#"{"a":1,"b":2}"#);   // "b" survives
```

## Repository layout

The **Rust core** lives at the repo root; each language ships from
`bindings/<lang>/`. The corpus is shared by all of them.

```
.
├── crates/safe-json-repair/   # Rust core: repair() + the 6-rung ladder
├── fuzz/                      # cargo-fuzz target for the core
├── corpus/cases.json          # the golden corpus — shared by EVERY binding
├── bindings/
│   └── js/                    # npm package — thin wrapper over the core via WASM
│       (python/ planned — see "Adding a binding" below)
└── PRD.md
```

| Language | Location | Install | Status |
|----------|----------|---------|--------|
| **Rust** | `crates/safe-json-repair/` | `cargo add safe-json-repair` | ✅ core |
| **JS/TS** | [`bindings/js/`](bindings/js/) | `npm i safe-json-repair` | ✅ shipped (WASM over the core) |
| **Python** | `bindings/python/` | — | 🔜 planned (PyO3 over the core) |

## The repair ladder

Strategies are tried in order; the first to yield a valid value wins, and the
chosen rung is reported back as `RepairResult::strategy`.

| Level | Strategy | Fixes |
|------:|----------|-------|
| 0 | `Parse` | valid JSON — returned verbatim, never rewritten |
| 1 | `StripFences` | ` ```json … ``` ` markdown wrappers |
| 2 | `StripControls` | literal control chars inside strings (`\n`/`\t`/`\r` preserved as escapes, others dropped) |
| 3 | `StripTrailingCommas` | `,}` / `,]` |
| 4 | `UnwrapDouble` | a JSON string that itself encodes an object/array |
| 5 | **`Tolerant`** | **stray/missing/mismatched closers; premature root close keeping siblings; truncation** |
| 6 | `Fallback` | everything failed → `null` or `{}` (configurable), **never throws** |

### Level 5 — the stack-aware tolerant parser (the soul of the library)

A hand-written recursive-descent parser that always returns *some* value:

1. **Mismatched closer → ask the ancestors.** A closer that doesn't match the
   current container is resolved against the open-container stack: if an
   *ancestor* owns it, the current container yields (repairs a *missing* own
   closer, e.g. `{"a":[1,2}` → `{"a":[1,2]}`); otherwise it's stray and skipped
   (repairs an *extra* closer, e.g. `{"a":1]}` → `{"a":1}`).
2. **Root has no siblings.** After the root value, a following `,` is impossible
   in valid JSON, so the preceding closer was spurious — the root is re-opened
   and sibling members keep being read. *This is what saves the orphaned key.*

It is string/escape aware, depth-bounded (`MAX_DEPTH = 120`, below `serde_json`'s
128 re-parse limit so output is always re-parseable), and guarantees forward
progress so it can never loop forever.

## Rust API

```rust
pub fn repair(input: &str, opts: &Options) -> RepairResult;

pub struct Options {
    pub max_len: usize,             // default 5_000_000; oversize → fallback
    pub fallback: Fallback,         // Null (default) | EmptyObject
    pub strip_code_fences: bool,    // default true
    pub unwrap_double_encoded: bool,// default true (object/array only)
}

pub struct RepairResult {
    pub ok: bool,            // false only on fallback
    pub value: serde_json::Value,
    pub json: String,        // canonical serialization of `value`
    pub changed: bool,       // was the input altered?
    pub strategy: Strategy,
}
```

## Quality

The golden corpus in [`corpus/cases.json`](corpus/cases.json) is the executable,
living spec. Each case asserts: recovers to valid JSON, equals the annotated
`expect` object key-for-key, and hits the expected strategy. **Every binding runs
the same file**, so all languages stay in lock-step.

```bash
cargo test                 # core: corpus + differential + robustness + doctests
(cd bindings/js && npm test)  # JS binding against the same corpus
```

* **Differential** — valid JSON must parse to exactly what `serde_json` /
  `JSON.parse` parses, report `Parse`, and never be marked `changed`.
* **Robustness** — deep nesting (100k), oversize input, pathological fragments,
  and 5 000 deterministic-fuzz mutations all return valid JSON without panicking
  or looping.
* **Fuzzing** (`fuzz/`) — `cargo +nightly fuzz run repair` asserts: never panics,
  output is always re-parseable, and valid JSON is never rewritten.
* **Competitor baseline** ([`bindings/js/test/baseline.test.ts`](bindings/js/test/baseline.test.ts))
  — runs the golden corpus through [`jsonrepair`](https://github.com/josdejong/jsonrepair)
  (~1.78M/wk, the ecosystem leader) and asserts we recover **more, never less**.
  Today we out-recover it on **3** cases (it mangles `C1`, throws on `C1-full`,
  and leaves the double-encoded `C6` un-unwrapped) and lose on **0**.

> ⚠️ The native test bar runs in the default **debug** profile. The wasm
> artifact uses a speed-tuned **release** profile (`opt-level = 3` + `wasm-opt
> -O3`; ~30% faster than `"z"`/`-Oz` for only ~0.5% more gzipped bytes) — build
> it with limited parallelism (`cargo build … -j 2`).

### Benchmarks

`crates/safe-json-repair/benches/repair.rs` measures the PRD performance targets
with [criterion](https://github.com/bheisler/criterion.rs). The `bench` profile
inherits the speed-tuned `release` profile (`opt-level = 3`) for realistic
native throughput.

```bash
cargo bench -j 2          # full run
cargo bench -j 2 --bench repair -- --measurement-time 3 --sample-size 30   # quick
```

Reference numbers (Apple Silicon, `opt-level = 3`):

| Scenario | Time | Target |
|---|---|---|
| Fast-path, valid 1 MB | ~9.0 ms (bare `serde_json` ~7.9 ms) | ≈ one `serde_json::from_str` |
| Tolerant repair, broken 1 MB | ~32.6 ms | < 50 ms |
| Real sample (`C1-full`) | ~19.7 µs/call | — |

## Adding a binding

Every binding wraps the **same Rust core** — no language reimplements the ladder
— and runs the **same `corpus/cases.json`** as its test spec, so nothing can
drift. A new language slots in as `bindings/<lang>/`:

* **JS/TS** (`bindings/js/`) — the core compiled to WebAssembly via
  `wasm-bindgen`, inlined into the npm bundle. Universal (Node/Bun/Deno/browser/
  edge), synchronous API.
* **Python** (planned, `bindings/python/`) — wrap the core with
  [PyO3](https://pyo3.rs) + [maturin](https://www.maturin.rs): a thin
  `#[pymodule]` over `safe_json_repair::repair`, `pyproject.toml`, and tests that
  load `../../corpus/cases.json`. Native (no wasm) for backend Python.

The principle: **logic lives once, in the core.** A fix to the parser lands in
every language at the next build.

## License

MIT
