// Competitor regression baseline (PRD §8.5 / Appendix A): feed the *same*
// golden corpus to the npm ecosystem leader and quantify exactly how much more
// we recover. This is the executable form of the PRD's claim "现成方案要么挂、
// 要么静默丢数据" — it asserts (a) we never recover *less* than a competitor on
// any case, and (b) we recover strictly *more* on the LLM-typical shapes the
// competitor mangles or throws on (C1 / C1-full — the v1 acceptance red-line).
//
// Currently baselined: jsonrepair (github.com/josdejong/jsonrepair, ~1.78M
// downloads/week), the library the PRD specifically calls out for avoidance.
// Adding another competitor is a one-line entry in COMPETITORS below.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { jsonrepair } from 'jsonrepair';
import { repairJson } from '../dist/index.js';

interface Case {
  name: string;
  desc: string;
  input: string;
  expect: unknown;
  strategy: string;
}

/** A competitor takes a broken string and returns a (hopefully) valid JSON
 * string, or throws. Mirrors the npm repair-lib contract. */
interface Competitor {
  name: string;
  repair: (input: string) => string;
}

const COMPETITORS: Competitor[] = [{ name: 'jsonrepair', repair: jsonrepair }];

type Outcome = 'match' | 'lossy' | 'error';

function loadCases(): Case[] {
  const path = fileURLToPath(new URL('../../../corpus/cases.json', import.meta.url));
  return JSON.parse(readFileSync(path, 'utf8')) as Case[];
}

function deepEqual(a: unknown, b: unknown): boolean {
  try {
    assert.deepStrictEqual(a, b);
    return true;
  } catch {
    return false;
  }
}

/** Classify a competitor's result against the annotated ground truth:
 *  - error: threw, or produced output that isn't even valid JSON
 *  - match: recovered exactly the expected value (no data lost)
 *  - lossy: produced valid JSON, but not the expected value (dropped/mangled) */
function classifyCompetitor(repair: (s: string) => string, input: string, expect: unknown): Outcome {
  let out: string;
  try {
    out = repair(input);
  } catch {
    return 'error';
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(out);
  } catch {
    return 'error';
  }
  return deepEqual(parsed, expect) ? 'match' : 'lossy';
}

test('competitor baseline: we recover more, never less', () => {
  // Only cases with a concrete expected value are comparable. fallback/any
  // cases (pure garbage, pathological depth) have no single ground truth.
  const cases = loadCases().filter((c) => c.strategy !== 'fallback' && c.strategy !== 'any');

  const rows: string[] = [];
  // Tally per competitor: how often we win / they win / both succeed.
  const tally = new Map<string, { oursWins: number; theyWin: number; bothMatch: number }>();
  for (const comp of COMPETITORS) tally.set(comp.name, { oursWins: 0, theyWin: 0, bothMatch: 0 });

  for (const c of cases) {
    // Ours is the ground-truth recoverer; the golden-corpus test already proves
    // value-exactness, so here we just read it back for the comparison table.
    const ours = repairJson(c.input);
    const ourMatch = ours.ok && deepEqual(ours.value, c.expect);

    const cells: string[] = [];
    for (const comp of COMPETITORS) {
      const outcome = classifyCompetitor(comp.repair, c.input, c.expect);
      cells.push(`${comp.name}=${outcome}`);

      const t = tally.get(comp.name)!;
      if (ourMatch && outcome === 'match') t.bothMatch++;
      else if (ourMatch && outcome !== 'match') t.oursWins++;
      else if (!ourMatch && outcome === 'match') t.theyWin++;

      // Invariant: we must never recover *less* than a competitor. Because the
      // golden corpus proves ourMatch === true for every deterministic case,
      // this can only fire if our core regressed — a useful tripwire.
      assert.ok(
        !(outcome === 'match' && !ourMatch),
        `${c.name}: regression — ${comp.name} recovered the expected value but we did not`,
      );
    }

    rows.push(`  ${c.name.padEnd(8)} ours=${ourMatch ? 'match' : 'MISS '}  ${cells.join('  ')}`);
  }

  // Emit the quantified baseline so CI logs record "how much more we save".
  console.log('\n── competitor baseline (golden corpus) ──');
  console.log(rows.join('\n'));
  for (const comp of COMPETITORS) {
    const t = tally.get(comp.name)!;
    console.log(
      `\n  vs ${comp.name}: we recover ${t.oursWins} case(s) it loses (throws/mangles), ` +
        `${t.bothMatch} both handle, ${t.theyWin} it-only.`,
    );
    // The differentiation must be real and non-trivial: at least one case we
    // recover that the competitor does not.
    assert.ok(t.oursWins >= 1, `expected to out-recover ${comp.name} on >=1 case`);
  }
});

// The v1 acceptance red-line, stated as a baseline contract: on the exact LLM
// shapes that motivated this library, the ecosystem leader fails and we don't.
test('red-line vs jsonrepair: C1 and C1-full', () => {
  const cases = loadCases();
  for (const name of ['C1', 'C1-full']) {
    const c = cases.find((x) => x.name === name)!;
    assert.ok(c, `${name} present`);

    // Ours: exact recovery.
    const ours = repairJson(c.input);
    assert.ok(ours.ok && deepEqual(ours.value, c.expect), `${name}: we must recover exactly`);

    // jsonrepair: must NOT produce the expected value (throws or mangles). If a
    // future jsonrepair version fixes this, this assertion fires — that's the
    // signal to re-evaluate our differentiation, not a silent pass.
    const outcome = classifyCompetitor(jsonrepair, c.input, c.expect);
    assert.notEqual(outcome, 'match', `${name}: jsonrepair unexpectedly recovered it — revisit baseline`);
  }
});
