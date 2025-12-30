"""Observer pattern implementation with async support.

RwLock's ``read()`` yields the underlying shared reference; treat it as
immutable unless you currently hold the write lock.
"""

from .core import IPublisher, IInnerObserverReceiver, Observer, Publisher
from .rwlock import RwLock

__all__ = [
    "RwLock",
    "IPublisher",
    "IInnerObserverReceiver",
    "Observer",
    "Publisher",
]
