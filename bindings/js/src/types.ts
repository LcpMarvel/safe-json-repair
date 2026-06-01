/**
 * Public types for `safe-json-repair`.
 *
 * The shape mirrors the Rust kernel one-to-one so the two bindings stay in
 * lock-step against the shared golden corpus (`corpus/cases.json`).
 */

/** Which rung of the repair ladder produced the result. */
export type Strategy =
  /** Level 0 — strict `JSON.parse` of valid JSON, returned verbatim. */
  | 'parse'
  /** Level 1 — Markdown code fence stripped. */
  | 'strip-fences'
  /** Level 2 — control characters inside strings tamed. */
  | 'strip-controls'
  /** Level 3 — trailing commas removed. */
  | 'strip-trailing-commas'
  /** Level 4 — double-encoded JSON string unwrapped. */
  | 'unwrap-double'
  /** Level 5 — stack-aware tolerant parser (the differentiator). */
  | 'tolerant'
  /** Level 6 — everything failed; the configured fallback was returned. */
  | 'fallback';

/** What to return when every repair strategy fails (corpus C11). */
export type Fallback =
  /** Return `null`. The general-purpose default — callers check for null. */
  | 'null'
  /** Return `{}`. For "must keep running" consumers like Tramito's backend. */
  | 'empty-object';

/** Tuning knobs for {@link repairJson}. All fields are optional. */
export interface RepairOptions {
  /**
   * Inputs longer than this (in UTF-16 code units) skip straight to the
   * fallback, guarding against pathological inputs. Default `5_000_000`.
   */
  maxLen?: number;
  /** What to return when nothing parses. Default `'null'`. */
  fallback?: Fallback;
  /** Strip Markdown code fences (level 1). Default `true`. */
  stripCodeFences?: boolean;
  /**
   * Unwrap a JSON string that itself encodes an object/array (level 4).
   * Default `true`.
   */
  unwrapDoubleEncoded?: boolean;
}

/** The result of a repair attempt. Always returned — {@link repairJson} never throws. */
export interface RepairResult<T = unknown> {
  /** Whether a meaningful value was recovered (`false` only for fallback). */
  ok: boolean;
  /** The recovered value (the fallback value when `ok` is false). */
  value: T;
  /** Canonical JSON serialization of {@link RepairResult.value}. */
  json: string;
  /** Whether the original text was altered to produce the result. */
  changed: boolean;
  /** Which ladder rung produced the result. */
  strategy: Strategy;
}

/** Defaults matching the PRD (and the Rust `Options::default`). */
export const DEFAULT_OPTIONS: Required<RepairOptions> = {
  maxLen: 5_000_000,
  fallback: 'null',
  stripCodeFences: true,
  unwrapDoubleEncoded: true,
};
