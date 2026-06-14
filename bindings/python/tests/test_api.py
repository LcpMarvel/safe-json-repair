"""API-surface tests for the Python binding: the public functions, options, the
result shape, and the never-throws contract."""

import json

import pytest

from safe_json_repair import (
    DEFAULT_OPTIONS,
    parse_json_safe,
    repair_json,
    repair_json_string,
)


def test_valid_json_fast_path():
    r = repair_json('{"a": 1, "b": [2, 3]}')
    assert r.ok
    assert r.strategy == "parse"
    assert r.changed is False
    assert r.value == {"a": 1, "b": [2, 3]}


def test_premature_root_close_keeps_sibling():
    r = repair_json('{"a":1}, "b":2}')
    assert r.ok
    assert r.strategy == "tolerant"
    assert r.value == {"a": 1, "b": 2}
    assert r.json == '{"a":1,"b":2}'
    assert r.changed is True


def test_result_value_is_native_python():
    r = repair_json('{"n": null, "f": 1.5, "t": true, "list": [1, "x"]}')
    assert isinstance(r.value, dict)
    assert r.value["n"] is None
    assert r.value["f"] == 1.5
    assert r.value["t"] is True
    assert r.value["list"] == [1, "x"]


def test_round_trips_through_json_loads():
    r = repair_json('```json\n{"a": 1,}\n```')
    assert r.ok
    assert json.loads(r.json) == r.value


def test_fallback_null_default():
    r = repair_json("not json at all !!!")
    assert not r.ok
    assert r.strategy == "fallback"
    assert r.value is None


def test_fallback_empty_object():
    r = repair_json("not json at all !!!", fallback="empty-object")
    assert not r.ok
    assert r.strategy == "fallback"
    assert r.value == {}


def test_invalid_fallback_raises():
    with pytest.raises(ValueError):
        repair_json("{}", fallback="nope")


def test_max_len_forces_fallback():
    r = repair_json('{"a": 1}', max_len=2)
    assert not r.ok
    assert r.strategy == "fallback"


def test_strip_code_fences_toggle():
    fenced = '```json\n{"a": 1}\n```'
    on = repair_json(fenced)
    assert on.ok
    assert on.value == {"a": 1}
    assert on.strategy == "strip-fences"
    # With fence-stripping off, the leading backticks defeat every rung and the
    # input falls through to the fallback — proof the option took effect.
    off = repair_json(fenced, strip_code_fences=False)
    assert not off.ok
    assert off.strategy == "fallback"


def test_unwrap_double_encoded_toggle():
    double = '"{\\"a\\": 1}"'
    on = repair_json(double)
    assert on.value == {"a": 1}
    assert on.strategy == "unwrap-double"
    off = repair_json(double, unwrap_double_encoded=False)
    assert off.value == "{\"a\": 1}"


def test_parse_json_safe():
    assert parse_json_safe('{"a": 1}') == {"a": 1}
    assert parse_json_safe("garbage !!!") is None


def test_repair_json_string():
    assert repair_json_string('{"a":1}, "b":2}') == '{"a":1,"b":2}'
    assert repair_json_string("garbage !!!") is None


def test_repr_is_informative():
    text = repr(repair_json('{"a": 1}'))
    assert text.startswith("RepairResult(")
    assert "ok=True" in text
    assert "parse" in text


def test_default_options_constant():
    assert DEFAULT_OPTIONS == {
        "max_len": 5_000_000,
        "fallback": "null",
        "strip_code_fences": True,
        "unwrap_double_encoded": True,
    }


def test_never_throws_on_arbitrary_bytes_as_text():
    for s in ["", "[", "{", "null", "}{", "\x00\x01", "[1,2,", '{"a":']:
        r = repair_json(s)
        # Always a deterministic result with valid JSON output.
        json.loads(r.json)
