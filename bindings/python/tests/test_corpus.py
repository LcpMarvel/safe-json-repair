"""The golden corpus is the executable, living PRD. Each case asserts the input
recovers to a valid JSON value, equals the annotated ``expect`` object
key-for-key, and hits the expected strategy. Loaded from the shared
``corpus/cases.json`` so the Rust, JS, and Python bindings run the very same
cases."""

import json
from pathlib import Path

import pytest

from safe_json_repair import repair_json

# Shared corpus lives at the repo root (../../../corpus from bindings/python/tests).
CORPUS = Path(__file__).resolve().parents[3] / "corpus" / "cases.json"


def load_cases():
    return json.loads(CORPUS.read_text(encoding="utf-8"))


def test_corpus_file_present():
    assert CORPUS.is_file(), f"corpus not found at {CORPUS}"


@pytest.mark.parametrize("case", load_cases(), ids=lambda c: c["name"])
def test_golden_corpus(case):
    result = repair_json(case["input"])

    # 1. Output is always valid JSON (round-trips through json.loads).
    json.loads(result.json)

    # 2. Strategy check (skip when annotated "any").
    if case["strategy"] != "any":
        assert result.strategy == case["strategy"], (
            f"{case['name']}: strategy expected {case['strategy']} got {result.strategy}"
        )

    # 3. Value check. For fallback/any we only assert the contract.
    if case["strategy"] == "fallback":
        assert not result.ok, f"{case['name']}: fallback case but ok=True"
        assert result.value is None, (
            f"{case['name']}: fallback expected None, got {result.value!r}"
        )
    elif case["strategy"] == "any":
        assert result.ok, f"{case['name']}: expected a recovered value, got fallback"
    else:
        assert result.value == case["expect"], (
            f"{case['name']}: value mismatch\n"
            f"  expected: {case['expect']!r}\n"
            f"  got:      {result.value!r}"
        )


def test_real_sample_keeps_summary():
    """The v1 acceptance red-line: the real-world broken sample must recover the
    full graph (4 lanes / 11 edges) **and keep the sibling ``summary``** — the
    exact field every existing library drops."""
    case = next(c for c in load_cases() if c["name"] == "C1-full")
    r = repair_json(case["input"])
    assert r.ok
    assert r.strategy == "tolerant"
    assert len(r.value["lanes"]) == 4, "all 4 lanes"
    assert len(r.value["edges"]) == 11, "all 11 edges"
    assert r.value["summary"] == case["expect"]["summary"], (
        "summary must survive the premature root close"
    )
