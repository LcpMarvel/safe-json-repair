// Coverage for the non-default options knobs (the corpus runs defaults).

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { repairJson, parseJsonSafe, repairJsonString } from '../dist/index.js';

test('fallback empty-object', () => {
  const r = repairJson('not json at all', { fallback: 'empty-object' });
  assert.equal(r.ok, false);
  assert.equal(r.strategy, 'fallback');
  assert.deepStrictEqual(r.value, {});
  assert.equal(r.json, '{}');
});

test('fallback null default', () => {
  const r = repairJson('not json at all');
  assert.equal(r.ok, false);
  assert.equal(r.value, null);
});

test('disable code fences', () => {
  // With fences disabled, level 1 is skipped; the tolerant parser still sees the
  // backticks as garbage but recovers the embedded object. Either way it must
  // not claim strip-fences.
  const r = repairJson('```json{"a":1}```', { stripCodeFences: false });
  assert.notEqual(r.strategy, 'strip-fences');
});

test('disable double unwrap keeps string', () => {
  const r = repairJson('"{\\"a\\":1}"', { unwrapDoubleEncoded: false });
  assert.equal(r.strategy, 'parse');
  assert.equal(r.value, '{"a":1}');
});

test('oversize input falls back without work', () => {
  // Perfectly valid JSON, but longer than maxLen → deterministic fallback.
  const r = repairJson('{"a":12345}', { maxLen: 8 });
  assert.equal(r.ok, false);
  assert.equal(r.strategy, 'fallback');
});

test('changed flag semantics', () => {
  assert.equal(repairJson('{"a":1}').changed, false); // clean parse
  assert.equal(repairJson('{"a":1,}').changed, true); // trailing comma
  assert.equal(repairJson('{"a":1]}').changed, true); // tolerant
});

test('convenience wrappers', () => {
  assert.deepStrictEqual(parseJsonSafe('{"a":1}, "b":2}'), { a: 1, b: 2 });
  assert.equal(parseJsonSafe('not json at all'), undefined);

  assert.equal(repairJsonString('{"a":1]}'), '{"a":1}');
  assert.equal(repairJsonString('not json at all'), null);
});
