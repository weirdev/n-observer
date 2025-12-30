from contextlib import asynccontextmanager
from asyncio import Lock, Condition
from typing import AsyncGenerator, Generic, TypeVar

T = TypeVar("T")
TI = TypeVar("TI")


class RwLock(Generic[T]):
    def __init__(self, value: T):
        self._rcond = Condition()
        self._wlock = Lock()
        self._readers = 0
        self._value = value

    @asynccontextmanager
    async def read(self) -> AsyncGenerator["T", None]:
        async with self._wlock:
            async with self._rcond:
                self._readers += 1
        yield self._value
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
