"""Lossless framing, including unknown commands and binary downloads.

Framing is neither rendering support nor an authorization decision.
"""

from ._native import (
    Element,
    ParseError,
    ParseResult,
    Syntax,
    parse,
)

__all__ = ["Element", "ParseError", "ParseResult", "Syntax", "parse"]
