"""Observer pattern implementation with async support.

RwLock's ``read()`` yields the underlying shared reference; treat it as
immutable unless you currently hold the write lock.
"""

from .n_observer import Observer, Publisher, RwLock

__all__ = [
    "Observer",
    "Publisher",
    "RwLock",
]
