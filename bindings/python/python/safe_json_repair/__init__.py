"""safe-json-repair — repair broken JSON from LLMs.

**Never throws**, **never silently drops a field**. One synchronous call, backed
by the same Rust core as the npm package (here via PyO3 — native, no wasm).

LLMs in tool-calling / structured-output mode routinely emit JSON with a stray
closing delimiter that closes the parent or root object too early, orphaning the
fields that follow. Other repair libraries throw or silently drop those fields;
this one keeps them.

    >>> from safe_json_repair import repair_json
    >>> r = repair_json('{"a":1}, "b":2}')   # a stray "}" closed the root early
    >>> r.ok
    True
    >>> r.value
    {'a': 1, 'b': 2}
    >>> r.json
    '{"a":1,"b":2}'
    >>> r.strategy
    'tolerant'
"""

from __future__ import annotations

from typing import Any, Literal, Optional, TypeVar

from ._safe_json_repair import RepairResult, __version__
from ._safe_json_repair import repair as _repair

__all__ = [
    "repair_json",
    "parse_json_safe",
    "repair_json_string",
    "RepairResult",
    "Fallback",
    "Strategy",
    "DEFAULT_OPTIONS",
    "__version__",
]

#: What to return when every repair strategy fails.
Fallback = Literal["null", "empty-object"]

#: Which rung of the repair ladder produced the result.
Strategy = Literal[
    "parse",
    "strip-fences",
    "strip-controls",
    "strip-trailing-commas",
    "unwrap-double",
    "tolerant",
    "fallback",
]

#: Defaults matching the PRD (and the Rust ``Options::default``).
DEFAULT_OPTIONS: dict[str, Any] = {
    "max_len": 5_000_000,
    "fallback": "null",
    "strip_code_fences": True,
    "unwrap_double_encoded": True,
}

T = TypeVar("T")


def repair_json(
    input: str,
    *,
    max_len: Optional[int] = None,
    fallback: Optional[Fallback] = None,
    strip_code_fences: Optional[bool] = None,
    unwrap_double_encoded: Optional[bool] = None,
) -> RepairResult:
    """Repair ``input`` into a valid JSON value, returning the full result.

    Never raises on input shape — any input, including pure garbage, returns a
    deterministic :class:`RepairResult`. Only an invalid *option* (e.g. an
    unknown ``fallback``) raises ``ValueError``.

    Options left as ``None`` use the core defaults (see ``DEFAULT_OPTIONS``).
    """
    return _repair(
        input,
        max_len=max_len,
        fallback=fallback,
        strip_code_fences=strip_code_fences,
        unwrap_double_encoded=unwrap_double_encoded,
    )


def parse_json_safe(
    input: str,
    *,
    max_len: Optional[int] = None,
    fallback: Optional[Fallback] = None,
    strip_code_fences: Optional[bool] = None,
    unwrap_double_encoded: Optional[bool] = None,
) -> Optional[Any]:
    """Parse ``input`` safely, returning the recovered value or ``None`` when
    nothing meaningful could be recovered. Never raises on input shape."""
    r = repair_json(
        input,
        max_len=max_len,
        fallback=fallback,
        strip_code_fences=strip_code_fences,
        unwrap_double_encoded=unwrap_double_encoded,
    )
    return r.value if r.ok else None


def repair_json_string(
    input: str,
    *,
    max_len: Optional[int] = None,
    fallback: Optional[Fallback] = None,
    strip_code_fences: Optional[bool] = None,
    unwrap_double_encoded: Optional[bool] = None,
) -> Optional[str]:
    """Return the canonical repaired JSON string, or ``None`` when nothing
    meaningful could be recovered. Never raises on input shape."""
    r = repair_json(
        input,
        max_len=max_len,
        fallback=fallback,
        strip_code_fences=strip_code_fences,
        unwrap_double_encoded=unwrap_double_encoded,
    )
    return r.json if r.ok else None
