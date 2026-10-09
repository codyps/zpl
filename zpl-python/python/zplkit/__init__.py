"""Local ZPL rendering. Text inputs are UTF-8; use bytes for printer encodings."""

from ._native import Options, __version__, library_version, render

__all__ = ["Options", "__version__", "library_version", "render"]
