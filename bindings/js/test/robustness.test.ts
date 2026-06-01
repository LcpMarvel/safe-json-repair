// Robustness: the library must return a deterministic, valid-JSON result for
// any input — including adversarial ones — without throwing, looping, or
// overflowing the stack.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { repairJson, type RepairOptions } from '../dist/index.js';

// Every result's `json` field must itself be parseable as JSON.
function assertValidResult(input: string): void {
  const r = repairJson(input);
  try {
    JSON.parse(r.json);
  } catch {
    assert.fail(`result.json not valid for input ${JSON.stringify(input)}: ${r.json}`);
  }
}

test('deep nesting does not overflow', () => {
  // Far deeper than MAX_DEPTH; must terminate without a stack overflow.
  assertValidResult('{"a":'.repeat(100_000));
  assertValidResult('['.repeat(100_000));

  // Balanced-but-huge depth.
  assertValidResult('['.repeat(50_000) + ']'.repeat(50_000));
});

test('long input is bounded', () => {
  // Within maxLen: handled.
  assertValidResult(`{"k":"${'x'.repeat(500_000)}"}`);

  // Over maxLen: deterministic fallback, no work attempted.
  const opts: RepairOptions = { maxLen: 1000 };
  const r = repairJson('x'.repeat(2000), opts);
  assert.equal(r.ok, false);
  assert.equal(r.value, null);
});

test('pathological fragments never throw', () => {
  const cases = [
    '',
    ' ',
    '\0',
    '﻿', // BOM only
    '{',
    '}',
    '[',
    ']',
    ':',
    ',',
    '"',
    '"\\',
    '"\\u',
    '"\\uXY',
    '"\\uD800', // lone high surrogate
    '{"a"',
    '{"a":',
    '{"a":,}',
    '{:}',
    '[,,,]',
    '}}}}}}',
    ']]]]]]',
    '{}{}{}{}',
    '[1,2,3',
    'truefalse',
    'NaN',
    'Infinity',
    '-',
    '1.2.3.4',
    '{"a":1}garbage trailing',
    '😀😀😀',
    '{"k":"v\nwith\treal\rcontrols"}',
  ];
  for (const c of cases) assertValidResult(c);
});

// Deterministic pseudo-random fuzz (LCG, no external rng). Generates byte soups
// and mutations of valid JSON; asserts every result is valid JSON.
test('deterministic fuzz', () => {
  const seedCorpus = [
    '{"a":1,"b":[2,3],"c":"text"}',
    '[{"x":1},{"y":2}]',
    '{"deep":{"er":{"est":true}}}',
    '{"s":"with \\"quotes\\" and \\\\ slashes"}',
  ];

  const MASK = (1n << 64n) - 1n;
  let state = 0x9e3779b97f4a7c15n;
  const next = (): number => {
    state = (state * 6364136223846793005n + 1442695040888963407n) & MASK;
    return Number((state >> 33n) & 0xffffffffn);
  };

  const encoder = new TextEncoder();
  const decoder = new TextDecoder(); // non-fatal: replaces invalid sequences
  const junk = [...'{}[],:"\\ \n\0'].map((c) => c.charCodeAt(0));

  for (let iter = 0; iter < 5000; iter++) {
    const base = seedCorpus[next() % seedCorpus.length]!;
    let bytes = Array.from(encoder.encode(base));

    const mutations = 1 + (next() % 6);
    for (let m = 0; m < mutations; m++) {
      if (bytes.length === 0) break;
      const op = next() % 4;
      const idx = next() % bytes.length;
      if (op === 0) {
        bytes.splice(idx, 1); // delete
      } else if (op === 1) {
        bytes.splice(idx, 0, junk[next() % junk.length]!); // insert
      } else if (op === 2) {
        bytes[idx] = next() % 256; // flip
      } else {
        bytes = bytes.slice(0, idx); // truncate
      }
    }

    const input = decoder.decode(new Uint8Array(bytes));
    const r = repairJson(input);
    try {
      JSON.parse(r.json);
    } catch {
      assert.fail(
        `iter ${iter}: invalid result.json ${JSON.stringify(r.json)} for input ${JSON.stringify(input)}`,
      );
    }
  }
});
