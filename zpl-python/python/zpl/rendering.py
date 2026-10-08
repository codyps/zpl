"""Rendering configuration and results; limits use the Rust library defaults."""

from ._native import (
    Compatibility,
    Document,
    RenderError,
)
from ._native import (
    RenderLimits as Limits,
)

__all__ = ["Compatibility", "Document", "Limits", "RenderError"]
