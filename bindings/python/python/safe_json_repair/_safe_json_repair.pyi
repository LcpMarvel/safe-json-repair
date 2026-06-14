"""Type stubs for the compiled PyO3 extension (`safe_json_repair._safe_json_repair`)."""

from typing import Any, Optional

__version__: str

class RepairResult:
    """The result of a repair attempt. Always returned — frozen/immutable."""

    ok: bool
    value: Any
    json: str
    changed: bool
    strategy: str
    def __repr__(self) -> str: ...

def repair(
    input: str,
    *,
    max_len: Optional[int] = ...,
    fallback: Optional[str] = ...,
    strip_code_fences: Optional[bool] = ...,
    unwrap_double_encoded: Optional[bool] = ...,
) -> RepairResult: ...
