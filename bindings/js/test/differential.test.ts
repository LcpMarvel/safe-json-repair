// Differential test: for valid JSON the library must behave like a fast-path —
// it parses to exactly what `JSON.parse` parses, reports strategy 'parse', and
// never marks the input as changed. Guards the PRD's "合法输入零改写" invariant
// (corpus C10) across a broad sample, not just one case.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { repairJson } from '../dist/index.js';

const VALID: string[] = [
  '{}',
  '[]',
  'null',
  'true',
  'false',
  '0',
  '-1',
  '3.14',
  '1e10',
  '-2.5e-3',
  '"hello"',
  '"with \\"escapes\\" and \\n newlines"',
  '"unicode é 😀"',
  '{"a":1,"b":2,"c":3}',
  '{"a":[1,2,3],"b":"x"}',
  '[{"x":1},{"y":2},{"z":3}]',
  '{"nested":{"deep":{"deeper":{"v":true}}}}',
  '{"o":{"a":1}, "b":2}',
  '{"items":[{"x":1},{"y":2}],"n":"t"}',
  '{"mixed":[1,"two",true,null,{"k":"v"},[9]]}',
  '{"empty_obj":{},"empty_arr":[],"empty_str":""}',
  '{"big":123456789012345,"neg":-987654321}',
  '   {"leading":"ws"}   ',
  '{"unicode_key_é":"value"}',
];

test('valid JSON is fast-path', () => {
  for (const input of VALID) {
    const r = repairJson(input);
    const expected = JSON.parse(input);

    assert.ok(r.ok, `${input}: expected ok`);
    assert.equal(r.strategy, 'parse', `${input}: expected fast-path parse`);
    assert.equal(r.changed, false, `${input}: fast-path must not mark changed`);
    assert.deepStrictEqual(r.value, expected, `${input}: value must equal JSON.parse`);

    // r.json must round-trip to the same value as the input.
    assert.deepStrictEqual(JSON.parse(r.json), expected, `${input}: r.json must round-trip`);
  }
});

// A valid JSON *string* that does not encode an object/array stays a string (no
// spurious double-unwrap). A string that *does* encode one is unwrapped.
test('unwrap only for object or array (D3)', () => {
  // Bare string -> stays a string (parse), not unwrapped.
  let r = repairJson('"just text"');
  assert.equal(r.strategy, 'parse');
  assert.equal(r.value, 'just text');

  // String encoding a number -> NOT unwrapped (D3: object/array only).
  r = repairJson('"42"');
  assert.equal(r.strategy, 'parse');
  assert.equal(r.value, '42');

  // String encoding an object -> unwrapped.
  r = repairJson('"{\\"a\\":1}"');
  assert.equal(r.strategy, 'unwrap-double');
  assert.deepStrictEqual(r.value, { a: 1 });
});
