# safe-json-repair

> Repair broken JSON — **never panics**, **never silently drops a field**.
> Zero runtime dependencies beyond `serde_json`.

LLMs in tool-calling / structured-output mode routinely emit *structurally*
broken JSON. The most common shape — and the one every existing repair library
mishandles — is a **stray closing delimiter that closes the parent or root
object too early**, orphaning the sibling keys that follow. This crate keeps
them.

```rust
use safe_json_repair::{repair, Options, Strategy};

// A stray `}` closed the root early; "b" would normally be lost.
let r = repair(r#"{"a":1}, "b":2}"#, &Options::default());
assert!(r.ok);
assert_eq!(r.strategy, Strategy::Tolerant);
assert_eq!(r.json, r#"{"a":1,"b":2}"#);   // "b" survives
```

```toml
[dependencies]
safe-json-repair = "0.1"
```

## How it works

`repair` runs a fixed ladder of strategies and returns at the first one that
yields valid JSON, reporting which rung fired via `RepairResult::strategy`:

| `Strategy` | Fixes |
|------------|-------|
| `Parse` | already valid JSON — returned verbatim, never rewritten |
| `StripFences` | ` ```json … ``` ` markdown wrappers |
| `StripControls` | literal control chars in strings (`\n`/`\t`/`\r` kept as escapes, others dropped) |
| `StripTrailingCommas` | `,}` / `,]` |
| `UnwrapDouble` | a JSON string that itself encodes an object/array |
| **`Tolerant`** | **stray / missing / mismatched closers; premature root close keeping siblings; truncation** |
| `Fallback` | nothing worked → `Null` or `{}` (configurable), **never panics** |

The `Tolerant` rung is the differentiator: a stack-aware recursive-descent
parser that, on a `}`/`]` which would close a container too early, decides
whether it belongs to an ancestor (yield) or is just stray (skip) — and knows
the root can't have a sibling, so a field after a premature root close is
reclaimed instead of lost. It is string/escape aware, depth-bounded, and
guarantees forward progress (it can never panic or loop forever).

## API

```rust
pub fn repair(input: &str, opts: &Options) -> RepairResult;

pub struct Options {
    pub max_len: usize,              // default 5_000_000; oversize → fallback
    pub fallback: Fallback,          // Null (default) | EmptyObject
    pub strip_code_fences: bool,     // default true
    pub unwrap_double_encoded: bool, // default true (object/array only)
}

pub struct RepairResult {
    pub ok: bool,                  // false only on fallback
    pub value: serde_json::Value,  // recovered value (fallback value when !ok)
    pub json: String,              // canonical serialization of `value`
    pub changed: bool,             // was the input altered?
    pub strategy: Strategy,
}
```

## Guarantees

* **Never panics.** Every input returns a `RepairResult`; garbage → `Fallback`.
* **Never silently drops data.** Siblings orphaned by a premature close are kept.
* **Never rewrites valid JSON.** Valid input takes the fast path: `Strategy::Parse`,
  `changed == false`, value identical to `serde_json::from_str`.
* **Always re-parseable.** `result.json` always parses back via `serde_json`.

This is the core that also powers the [`safe-json-repair` npm
package](https://www.npmjs.com/package/safe-json-repair) (the same logic
compiled to WebAssembly), so behavior is identical across languages — both run
the same golden test corpus.

## License

MIT © LcpMarvel
