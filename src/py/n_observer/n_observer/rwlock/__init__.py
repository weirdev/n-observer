from contextlib import asynccontextmanager
from asyncio import Lock, Condition
from typing import AsyncGenerator, Generic, TypeVar

T = TypeVar("T")
TI = TypeVar("TI")


class RwLock(Generic[T]):
    def __init__(self, value: T):
        self._rlock = Lock()
        self._wlock = Lock()
        self._cond = Condition()
        self._readers = 0
        self._value = value

    @asynccontextmanager
    async def read(self) -> AsyncGenerator["T", None]:
        async with self._wlock:
            # while self._rlock.locked():
            #     async with self._cond:
            #         await self._cond.wait()
            async with self._rlock:
                self._readers += 1
        yield self._value
        async with self._rlock:
            self._readers -= 1
            if self._readers == 0:
                async with self._cond:
                    self._cond.notify()

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
            reading = True
            while reading:
                await self._rlock.acquire()
                reading = self._readers > 0
                if reading:
                    async with self._cond:
                        # Release read lock so readers can finish
                        self._rlock.release()
                        # Wait for readers to finish
                        await self._cond.wait()
                else:
                    self._rlock.release()
                    break
            yield RwLock.Writer(self)
            # async with self._cond:
            #     self._cond.notify_all()
