# safe-json-repair

> Repair broken JSON from LLMs — **never throws**, **never silently drops a
> field**. One synchronous call, runs everywhere (Node, Bun, Deno, browsers,
> edge).

```bash
npm i safe-json-repair      # or: bun add / pnpm add / yarn add
```

```ts
import { repairJson } from 'safe-json-repair';

const r = repairJson('{"a":1}, "b":2}');   // a stray "}" closed the root early
r.ok;        // true
r.value;     // { a: 1, b: 2 }      ← "b" is NOT lost
r.json;      // '{"a":1,"b":2}'
r.strategy;  // 'tolerant'
```

## What problem this solves

LLMs in tool-calling / structured-output mode (DeepSeek, OpenAI, …) routinely
return JSON with a **stray closing delimiter that closes the parent or root
object too early**, orphaning every field that comes after it. On that exact
shape, the popular repair libraries fail:

| Library | On `{"a":1}, "b":2}` |
|---------|----------------------|
| [`jsonrepair`](https://github.com/josdejong/jsonrepair) (1.78M/wk) | throws or mangles it |
| `@qraftr/json-repair`, `jaison`, `json-repair-js` | recover structure but **silently drop `"b"`** |
| **`safe-json-repair`** | **keeps `"b"`** ✅ |

Dropping a field is the dangerous failure: your tool call silently loses data
and you never find out. This library is built so that never happens.

## Usage

### The full result

```ts
import { repairJson } from 'safe-json-repair';

const r = repairJson(maybeBrokenJson);

if (r.ok) {
  doSomething(r.value);          // recovered value (already a JS object)
} else {
  // Only happens for unrepairable garbage. r.value is the fallback (null by default).
  log.warn('could not repair JSON', { strategy: r.strategy });
}
```

### Convenience wrappers

```ts
import { parseJsonSafe, repairJsonString } from 'safe-json-repair';

// Just the value, typed — or `undefined` if nothing could be recovered.
const args = parseJsonSafe<ToolArgs>(brokenToolArgs);

// Just the repaired JSON string — or `null`.
const json = repairJsonString(brokenToolArgs);
```

### Real-world: guarding an LLM tool call

```ts
import { parseJsonSafe } from 'safe-json-repair';

function readToolArgs(raw: string): ToolArgs {
  // The model usually returns valid JSON (fast path, zero rewrite). When it
  // doesn't, we recover instead of crashing the whole turn — and we keep every
  // field, so the downstream tool gets complete arguments.
  const args = parseJsonSafe<ToolArgs>(raw, { fallback: 'empty-object' });
  if (!args) throw new Error('tool args unrecoverable');
  return args;
}
```

`repairJson` is **synchronous** and **never throws** — the WebAssembly module is
compiled once, lazily, on the first call. Any input, including pure garbage,
returns a deterministic `RepairResult`.

## API

```ts
function repairJson(input: string, options?: RepairOptions): RepairResult;
function parseJsonSafe<T = unknown>(input: string, options?: RepairOptions): T | undefined;
function repairJsonString(input: string, options?: RepairOptions): string | null;

interface RepairOptions {
  maxLen?: number;                     // default 5_000_000; larger input → fallback (no work)
  fallback?: 'null' | 'empty-object';  // what to return when nothing parses. default 'null'
  stripCodeFences?: boolean;           // strip ```json … ``` wrappers. default true
  unwrapDoubleEncoded?: boolean;       // unwrap a JSON string that holds an object/array. default true
}

interface RepairResult<T = unknown> {
  ok: boolean;        // false only when it fell through to fallback
  value: T;           // the recovered value (the fallback value when !ok)
  json: string;       // canonical JSON serialization of `value`
  changed: boolean;   // was the original input altered to produce this?
  strategy: Strategy; // which rung of the ladder produced the result
}

type Strategy =
  | 'parse' | 'strip-fences' | 'strip-controls'
  | 'strip-trailing-commas' | 'unwrap-double' | 'tolerant' | 'fallback';
```

### Options in practice

```ts
// Keep your app running on any input: return {} instead of null when unrepairable.
repairJson(raw, { fallback: 'empty-object' });

// A field whose string value just happens to look like JSON? Don't unwrap it.
repairJson(raw, { unwrapDoubleEncoded: false });
```

## How it fixes things

It tries a fixed ladder of strategies and returns at the first one that yields
valid JSON, telling you which rung fired via `result.strategy`:

| `strategy` | Fixes |
|------------|-------|
| `'parse'` | already valid JSON — returned **verbatim**, never rewritten |
| `'strip-fences'` | ` ```json … ``` ` markdown wrappers |
| `'strip-controls'` | literal control chars in strings (`\n`/`\t`/`\r` kept as escapes, others dropped) |
| `'strip-trailing-commas'` | `,}` / `,]` |
| `'unwrap-double'` | a JSON string that itself encodes an object/array |
| **`'tolerant'`** | **stray / missing / mismatched closers; premature root close keeping siblings; truncation** |
| `'fallback'` | nothing worked → `null` or `{}` (your choice), **never throws** |

The `'tolerant'` rung is the differentiator: a stack-aware parser that, when a
`}`/`]` would close a container too early, decides whether it belongs to an
ancestor (yield) or is just stray (skip) — and knows the root can't have a
sibling, so a field after a premature root close is reclaimed instead of lost.

## Guarantees

* **Never throws.** Every input returns a `RepairResult`. Garbage → `'fallback'`.
* **Never silently drops data.** Siblings orphaned by a premature close are kept.
* **Never rewrites valid JSON.** Valid input takes the fast path: `strategy:
  'parse'`, `changed: false`, value identical to `JSON.parse`.
* **Always re-parseable.** `result.json` is always valid JSON.

## Runtime & size

* **Universal, zero config.** One artifact runs in Node, Bun, Deno, browsers,
  and Cloudflare / Vercel Edge. The WebAssembly is inlined into the bundle and
  instantiated via `initSync` — no `fetch`, no `fs`, no bundler/Vite/webpack
  setup, no `postinstall`.
* **~57 KB gzipped** (the wasm carries a full Rust JSON engine). Speed-tuned: on
  valid 1 MB input it's on par with / faster than a pure-JS repairer.
* **Browser CSP:** compiling WebAssembly needs `'wasm-unsafe-eval'` in the
  page's Content-Security-Policy (most pages don't restrict it). Node / Bun /
  Deno / edge are unaffected.

## How it's built

This package is a thin wrapper over the **same Rust core** published as the
[`safe-json-repair` crate](https://crates.io/crates/safe-json-repair), compiled
to WebAssembly — there is no second TypeScript reimplementation to drift out of
sync. Both sides run the same golden test corpus.

## License

MIT © LcpMarvel
