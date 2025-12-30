from contextlib import asynccontextmanager
from asyncio import Lock, Condition
from typing import AsyncGenerator, Generic, TypeVar

T = TypeVar("T")
TI = TypeVar("TI")


class RwLock(Generic[T]):
    """Async reader-writer lock around a single shared value.

    The value returned by :meth:`read` is the underlying reference; callers must
    treat it as immutable unless they hold the write lock.

    Example with a mutable value (replace rather than mutate in-place):

    >>> async with lock.read() as items:
    ...     snapshot = items
    >>> async with lock.write() as writer:
    ...     writer.set_value(snapshot + ["new-item"])

    Note that once a writer acquires the write lock, no new readers can acquire
    the read lock until the writer releases the write lock.
    However, in highly contended scenarios, it is theoretically possible for readers
    to starve writers indefinitely. RwLock has the same fairness properties as asyncio.Lock.
    """

    def __init__(self, value: T):
        self._rcond = Condition()
        self._wlock = Lock()
        self._readers = 0
        self._value = value

    @asynccontextmanager
    async def read(self) -> AsyncGenerator["T", None]:
        """Acquire a read lock and yield the underlying value.

        The yielded object is the shared reference; treat it as immutable unless
        you currently hold the write lock.
        """
        async with self._wlock:
            async with self._rcond:
                self._readers += 1
        try:
            yield self._value
        finally:
            async with self._rcond:
                self._readers -= 1
                if self._readers == 0:
                    self._rcond.notify()

    class Writer(Generic[TI]):
        def __init__(self, rwlock: "RwLock[TI]"):
            self._rwlock = rwlock

        def get_value(
            self,
        ) -> TI:
            return self._rwlock._value

        def set_value(
            self,
            value: TI,
        ) -> None:
            self._rwlock._value = value

    @asynccontextmanager
    async def write(self) -> AsyncGenerator["RwLock[T].Writer[T]", None]:
        async with self._wlock:
            async with self._rcond:
                while self._readers > 0:
                    await self._rcond.wait()
            yield RwLock.Writer(self)
