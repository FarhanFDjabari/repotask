"""Project fact indexing."""

from repotask.index.families import build_all, collect
from repotask.index.runner import IndexResult, Symbol, build_index

__all__ = ["IndexResult", "Symbol", "build_all", "build_index", "collect"]
