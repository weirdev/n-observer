from contextlib import asynccontextmanager
import asyncio
from asyncio import Lock, Condition
from typing import Callable, AsyncGenerator, Optional, TypeVar, Generic
from typing_extensions import override
import typing

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


class IPublisher:
    async def add_observer(
        self, observer: "IInnerObserverReceiver", input_index: int
    ) -> Optional[object]:
        """s
        Add an observer to the publisher.
        """
        raise NotImplementedError

    async def notify(self, value: object) -> None:
        """
        Notify all observers with a new value.
        """
        raise NotImplementedError


class IInnerObserverReceiver:
    async def update(self, data: list[Optional[object]]) -> None:
        """
        Update the observer with a future value.
        """
        raise NotImplementedError


class IObservable(IPublisher, IInnerObserverReceiver, Generic[T]):
    async def get(self) -> Optional[T]:
        """
        Get the current value of the observable.
        """
        raise NotImplementedError


class Observer(IObservable[T], Generic[T]):
    def __init__(self, inner: "InnerObserverImpl[T]") -> None:
        """
        Private constructor.
        """
        self.inner = inner

    @staticmethod
    async def new(publisher: "IPublisher") -> "Observer[T]":
        return await Observer[T].new_with_transform(
            publisher,
            lambda xs: typing.cast(T, xs),
        )

    @classmethod
    async def new_with_transform(
        cls, publisher: "IPublisher", transform: Callable[[object], T]
    ) -> "Observer[T]":
        self_publisher: Publisher = Publisher()
        current: RwLock[Optional[T]] = RwLock(None)
        last_inputs: RwLock[list[Optional[object]]] = RwLock([None])
        inner: InnerObserverImpl[T] = InnerObserverImpl(
            current,
            self_publisher,
            lambda inputs: transform(inputs[0]),
            last_inputs,
        )

        initial_value = await publisher.add_observer(inner, 0)
        if initial_value is not None:
            await inner.update([initial_value])

        return cls(inner)

    @classmethod
    async def new_multiparent(
        cls,
        publishers: list[IPublisher],
        transform: Callable[[list[object]], T],
    ) -> "Observer[T]":
        self_publisher: Publisher = Publisher()
        current: RwLock[Optional[T]] = RwLock(None)
        last_inputs: RwLock[list[Optional[object]]] = RwLock([None] * len(publishers))
        inner: InnerObserverImpl[T] = InnerObserverImpl(
            current,
            self_publisher,
            transform,
            last_inputs,
        )

        initial_values = [
            await publisher.add_observer(inner, i)
            for i, publisher in enumerate(publishers)
        ]
        await inner.update(initial_values)

        return cls(inner)

    @override
    async def get(self) -> Optional[T]:
        return await self.inner.get()

    @override
    async def add_observer(
        self, observer: "IInnerObserverReceiver", input_index: int
    ) -> Optional[object]:
        return await self.inner.publisher.add_observer(observer, input_index)

    @override
    async def notify(self, value: object) -> None:
        await self.inner.publisher.notify(value)

    @override
    async def update(self, data: list[Optional[object]]) -> None:
        """
        Passthrough to inner observer.
        """
        await self.inner.update(data)


class InnerObserverImpl(IInnerObserverReceiver, Generic[T]):
    def __init__(
        self,
        current: RwLock[Optional[T]],
        publisher: "Publisher",
        transform: Callable[[list[object]], T],
        last_inputs: RwLock[list[Optional[object]]],
    ) -> None:
        self.current = current
        self.publisher = publisher
        self.transform = transform
        self.last_inputs = last_inputs

    async def get(self) -> Optional[T]:
        async with self.current.read() as rvalue:
            return rvalue

    @override
    async def update(self, data: list[Optional[object]]) -> None:
        await self.update_with_transform(data)

    async def update_direct(self, value: T) -> None:
        """
        NOTE: `last_inputs` must be updated before calling this
        """
        async with self.current.write() as writer:
            writer.set_value(value)
        await self.publisher.notify(value)

    async def update_with_transform(self, value: list[Optional[object]]) -> None:
        async with self.last_inputs.write() as last_inputs_writer:
            last_inputs = last_inputs_writer.get_value()
            inputs = []
            for i, (input, _last_input) in enumerate(
                zip(value, last_inputs, strict=False)
            ):
                if input is not None:
                    last_inputs[i] = input

            inputs = [input for input in last_inputs if input is not None]
            if len(inputs) != len(last_inputs):
                return

        try:
            transformed_value = self.transform(inputs)
            await self.update_direct(transformed_value)
        except Exception:
            pass


class Publisher(IPublisher):
    def __init__(self, initial_value: Optional[object] = None) -> None:
        observers: list[tuple[int, IInnerObserverReceiver]] = []
        self._observers: RwLock[list[tuple[int, IInnerObserverReceiver]]] = RwLock(
            observers
        )
        self._current = RwLock(initial_value)

    @override
    async def add_observer(
        self, observer: IInnerObserverReceiver, input_index: int
    ) -> Optional[object]:
        async with self._observers.write() as writer:
            writer.get_value().append((input_index, observer))

        async with self._current.read() as rvalue:
            return rvalue

    @override
    async def notify(self, value: object) -> None:
        async with self._current.write() as writer:
            writer.set_value(value)

        async with self._observers.read() as rvalue:
            updates = []
            for i, observer in rvalue:
                inputs: list[Optional[object]] = [None] * (i + 1)
                inputs[i] = value
                updates.append(observer.update(inputs))
            if updates:
                results = await asyncio.gather(*updates, return_exceptions=True)
                for result in results:
                    if isinstance(result, Exception):
                        raise result
