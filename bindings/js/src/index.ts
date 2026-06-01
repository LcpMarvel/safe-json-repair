/**
 * safe-json-repair — a JSON repair library that **never throws** and **never
 * silently drops data**.
 *
 * This package is a thin wrapper around the **Rust core** compiled to WebAssembly
 * (inlined as bytes — no fetch, no fs, no bundler config). All repair logic lives
 * once, in Rust; there is no second implementation to drift. Runs in Node, Bun,
 * Deno, browsers, and edge runtimes with a synchronous API.
 *
 * @example
 * ```ts
 * import { repairJson } from 'safe-json-repair';
 *
 * // Premature root close + sibling key (the real-world DeepSeek shape).
 * const r = repairJson('{"a":1}, "b":2}');
 * r.ok;        // true
 * r.strategy;  // 'tolerant'
 * r.json;      // '{"a":1,"b":2}'  ← "b" survives
 * ```
 */

import { repair as wasmRepair, initSync } from './generated/glue.js';
import { WASM_BASE64 } from './generated/wasm-inline.js';
import type { RepairOptions, RepairResult } from './types.js';

export {
  DEFAULT_OPTIONS,
  type Fallback,
  type RepairOptions,
  type RepairResult,
  type Strategy,
} from './types.js';

let initialized = false;

/** Decode base64 to bytes in any runtime (Node/Bun Buffer, else atob). */
function base64ToBytes(b64: string): Uint8Array {
  const g = globalThis as unknown as {
    Buffer?: { from(s: string, enc: string): Uint8Array };
    atob?: (s: string) => string;
  };
  if (typeof g.Buffer !== 'undefined') {
    return new Uint8Array(g.Buffer.from(b64, 'base64'));
  }
  if (typeof g.atob === 'function') {
    const bin = g.atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return bytes;
  }
  throw new Error('safe-json-repair: no base64 decoder (Buffer/atob) available in this runtime');
}

/** Compile + instantiate the wasm module once, lazily, on first use. */
function ensureInit(): void {
  if (initialized) return;
  initSync({ module: base64ToBytes(WASM_BASE64) });
  initialized = true;
}

/**
 * Repair `input` into a valid JSON value. Never throws — any input, including
 * pure garbage, returns a deterministic {@link RepairResult}.
 */
export function repairJson(input: string, options?: RepairOptions): RepairResult {
  ensureInit();
  return wasmRepair(input, options) as RepairResult;
}

/**
 * Convenience wrapper: parse `input` safely, returning the recovered value or
 * `undefined` when nothing meaningful could be recovered. Never throws.
 */
export function parseJsonSafe<T = unknown>(
  input: string,
  options?: RepairOptions,
): T | undefined {
  const r = repairJson(input, options);
  return r.ok ? (r.value as T) : undefined;
}

/**
 * Convenience wrapper: return the canonical repaired JSON string, or `null`
 * when nothing meaningful could be recovered. Never throws.
 */
export function repairJsonString(input: string, options?: RepairOptions): string | null {
  const r = repairJson(input, options);
  return r.ok ? r.json : null;
}
