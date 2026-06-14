# safe-json-repair (Python)

> Repair broken JSON from LLMs — **never throws**, **never silently drops a
> field**. One synchronous call, native (PyO3 over the Rust core — no wasm).

```bash
pip install safe-json-repair        # or: uv add safe-json-repair
```

```python
from safe_json_repair import repair_json

r = repair_json('{"a":1}, "b":2}')   # a stray "}" closed the root early
r.ok        # True
r.value     # {'a': 1, 'b': 2}     ← "b" is NOT lost
r.json      # '{"a":1,"b":2}'
r.strategy  # 'tolerant'
```

## What problem this solves

LLMs in tool-calling / structured-output mode (DeepSeek, OpenAI, …) routinely
return JSON with a **stray closing delimiter that closes the parent or root
object too early**, orphaning every field that comes after it. On that exact
shape the popular repair libraries either throw or **silently drop** the
orphaned fields — and a silently dropped field is the dangerous failure: your
tool call loses data and you never find out. This library keeps it.

## Usage

### The full result

```python
from safe_json_repair import repair_json

r = repair_json(maybe_broken_json)

if r.ok:
    do_something(r.value)          # recovered value (already a dict/list/…)
else:
    # Only for unrepairable garbage. r.value is the fallback (None by default).
    log.warning("could not repair JSON: strategy=%s", r.strategy)
```

### Convenience wrappers

```python
from safe_json_repair import parse_json_safe, repair_json_string

# Just the value — or None if nothing could be recovered.
args = parse_json_safe(broken_tool_args)

# Just the repaired JSON string — or None.
text = repair_json_string(broken_tool_args)
```

### Real-world: guarding an LLM tool call

```python
from safe_json_repair import parse_json_safe

def read_tool_args(raw: str) -> dict:
    # Usually valid JSON (fast path, zero rewrite). When it isn't, we recover
    # instead of crashing the turn — and keep every field, so the downstream
    # tool gets complete arguments.
    args = parse_json_safe(raw, fallback="empty-object")
    if args is None:
        raise ValueError("tool args unrecoverable")
    return args
```

`repair_json` is **synchronous** and **never raises on input shape** — any
input, including pure garbage, returns a deterministic `RepairResult`. (Only an
invalid *option*, such as an unknown `fallback`, raises `ValueError`.)

## API

```python
def repair_json(input: str, *, max_len=None, fallback=None,
                strip_code_fences=None, unwrap_double_encoded=None) -> RepairResult: ...
def parse_json_safe(input: str, **options) -> Any | None: ...
def repair_json_string(input: str, **options) -> str | None: ...
```

Options (each `None` ⇒ the core default):

| Option | Default | Meaning |
|--------|---------|---------|
| `max_len` | `5_000_000` | inputs longer than this (bytes) skip to the fallback |
| `fallback` | `"null"` | what to return when nothing parses: `"null"` or `"empty-object"` |
| `strip_code_fences` | `True` | strip ` ```json … ``` ` wrappers |
| `unwrap_double_encoded` | `True` | unwrap a JSON string that itself encodes an object/array |

`RepairResult` (immutable):

| Attribute | Type | Meaning |
|-----------|------|---------|
| `ok` | `bool` | `False` only when it fell through to the fallback |
| `value` | `Any` | the recovered value (the fallback value when `not ok`) |
| `json` | `str` | canonical JSON serialization of `value` (always re-parseable) |
| `changed` | `bool` | was the original input altered to produce this? |
| `strategy` | `str` | which rung of the ladder fired (see below) |

`strategy` is one of: `'parse'`, `'strip-fences'`, `'strip-controls'`,
`'strip-trailing-commas'`, `'unwrap-double'`, `'tolerant'`, `'fallback'`.

## Guarantees

* **Never throws on input.** Every input returns a `RepairResult`. Garbage → `'fallback'`.
* **Never silently drops data.** Siblings orphaned by a premature close are kept.
* **Never rewrites valid JSON.** Valid input takes the fast path: `strategy='parse'`,
  `changed=False`, value identical to `json.loads`.
* **Always re-parseable.** `result.json` is always valid JSON.

## How it's built

This package is a thin [PyO3](https://pyo3.rs) wrapper over the **same Rust
core** published as the [`safe-json-repair` crate](https://crates.io/crates/safe-json-repair)
and the [npm package](https://www.npmjs.com/package/safe-json-repair) — there is
no second reimplementation to drift out of sync. All three bindings run the same
golden corpus (`corpus/cases.json`). Native code, no wasm.

### Building from source

```bash
cd bindings/python
uv venv && uv pip install maturin pytest
maturin develop        # compile + install into the venv
pytest                 # runs the shared golden corpus + API tests
```

## License

MIT © LcpMarvel
