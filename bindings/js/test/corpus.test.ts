// The golden corpus is the executable, living PRD. Each case asserts the input
// recovers to a valid JSON value, equals the annotated `expect` object
// key-for-key, and hits the expected strategy. Loaded from the shared
// `corpus/cases.json` so the Rust and TS bindings run the very same cases.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { repairJson } from '../dist/index.js';

interface Case {
  name: string;
  desc: string;
  input: string;
  expect: unknown;
  strategy: string;
}

function loadCases(): Case[] {
  // Shared corpus lives at the repo root (../../../corpus from bindings/js/test).
  const path = fileURLToPath(new URL('../../../corpus/cases.json', import.meta.url));
  return JSON.parse(readFileSync(path, 'utf8')) as Case[];
}

test('golden corpus', () => {
  const failures: string[] = [];

  for (const c of loadCases()) {
    const result = repairJson(c.input);

    // 1. Output is always valid JSON (round-trips through JSON.parse).
    try {
      JSON.parse(result.json);
    } catch {
      failures.push(`${c.name}: result.json is not valid JSON: ${result.json}`);
      continue;
    }

    // 2. Strategy check (skip when annotated "any").
    if (c.strategy !== 'any' && result.strategy !== c.strategy) {
      failures.push(`${c.name}: strategy expected ${c.strategy} got ${result.strategy}`);
    }

    // 3. Value check. For fallback/any we only assert the contract.
    if (c.strategy === 'fallback') {
      if (result.ok) failures.push(`${c.name}: fallback case but ok=true`);
      if (result.value !== null) {
        failures.push(`${c.name}: fallback expected null, got ${JSON.stringify(result.value)}`);
      }
    } else if (c.strategy === 'any') {
      if (!result.ok) failures.push(`${c.name}: expected a recovered value, got fallback`);
    } else {
      try {
        assert.deepStrictEqual(result.value, c.expect);
      } catch {
        failures.push(
          `${c.name}: value mismatch\n  expected: ${JSON.stringify(c.expect)}\n  got:      ${JSON.stringify(result.value)}`,
        );
      }
    }
  }

  assert.equal(failures.length, 0, `corpus failures:\n${failures.join('\n')}`);
});

// The v1 acceptance red-line: the real-world broken sample must recover the
// full graph (4 lanes / 11 edges) **and keep the sibling `summary`** — the exact
// field every existing library drops.
test('real sample keeps summary (C1-full)', () => {
  const c = loadCases().find((x) => x.name === 'C1-full');
  assert.ok(c, 'C1-full present');
  const r = repairJson(c!.input);
  assert.ok(r.ok);
  assert.equal(r.strategy, 'tolerant');
  const obj = r.value as Record<string, unknown>;
  assert.equal((obj.lanes as unknown[]).length, 4, 'all 4 lanes');
  assert.equal((obj.edges as unknown[]).length, 11, 'all 11 edges');
  assert.deepStrictEqual(
    obj.summary,
    (c!.expect as Record<string, unknown>).summary,
    'summary must survive the premature root close',
  );
});
