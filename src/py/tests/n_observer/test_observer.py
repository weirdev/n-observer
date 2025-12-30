from typing import Any, Optional
import asyncio
import typing
import pytest

from n_observer.n_observer import IInnerObserverReceiver, Observer, Publisher, RwLock


@pytest.mark.asyncio
async def test_passthrough_observer() -> None:
    publisher = Publisher()
    observer = await Observer[int].new(publisher)

    await publisher.notify(5)
    assert await observer.get() == 5

    await publisher.notify(10)
    assert await observer.get() == 10

    await publisher.notify(0)
    assert await observer.get() == 0


@pytest.mark.asyncio
async def test_homogenous_observer() -> None:
    def transform(x: object) -> int:
        x = typing.cast(int, x)
        return x * 2

    publisher = Publisher()
    observer = await Observer[int].new_with_transform(publisher, transform)

    await publisher.notify(5)
    assert await observer.get() == 10

    await publisher.notify(10)
    assert await observer.get() == 20

    await publisher.notify(0)
    assert await observer.get() == 0


@pytest.mark.asyncio
async def test_heterogeneous_observer() -> None:
    def transform(x: object) -> str:
        return str(typing.cast(int, x) * 2)

    publisher = Publisher()
    observer = await Observer[str].new_with_transform(publisher, transform)

    await publisher.notify(5)
    assert await observer.get() == "10"

    await publisher.notify(10)
    assert await observer.get() == "20"

    await publisher.notify(0)
    assert await observer.get() == "0"


@pytest.mark.asyncio
async def test_observer_multiple_publishers() -> None:
    def transform(x: list[object]) -> int:
        data1 = typing.cast(str, x[0])
        data2 = typing.cast(str, x[1])
        return len(data1) + len(data2)

    publisher1 = Publisher()
    publisher2 = Publisher()
    observer = await Observer[int].new_multiparent([publisher1, publisher2], transform)

    await publisher1.notify("Hello")
    await publisher2.notify("World!")

    assert await observer.get() == 11

    await publisher1.notify("Centconf")
    assert await observer.get() == 14


@pytest.mark.asyncio
async def test_chain_observers() -> None:
    def transform1(x: object) -> int:
        return typing.cast(int, x) + 1

    def transform2(x: object) -> str:
        return str(typing.cast(int, x) * 2)

    publisher = Publisher()
    observer1 = await Observer[int].new_with_transform(publisher, transform1)
    observer2 = await Observer[str].new_with_transform(observer1, transform2)

    await publisher.notify(5)
    assert await observer2.get() == "12"

    await publisher.notify(10)
    assert await observer2.get() == "22"

    await publisher.notify(0)
    assert await observer2.get() == "2"


@pytest.mark.asyncio
async def test_rwlock() -> None:
    rwlock = RwLock(0)

    async def reader(yield_while_holding_lock: bool = False) -> int:
        async with rwlock.read() as value:
            if yield_while_holding_lock:
                await asyncio.sleep(0)
            return value

    async def writer(new_value, yield_while_holding_lock: bool = False) -> None:
        async with rwlock.write() as writer:
            if yield_while_holding_lock:
                await asyncio.sleep(0)
            writer.set_value(new_value)

    assert await reader() == 0

    await writer(42)
    assert await reader() == 42

    async def concurrent_read() -> int:
        return await reader()

    await asyncio.gather(reader(), concurrent_read())

    async def concurrent_writer(new_value) -> None:
        await writer(new_value)

    await asyncio.gather(writer(42), concurrent_writer(100))

    assert await reader() == 100

    async def read_during_write() -> None:
        nonlocal rwlock
        async with rwlock.write() as _writer:
            try:
                await asyncio.wait_for(reader(), timeout=0.1)
                raise RuntimeError("Read should block during write")
            except TimeoutError:
                rwlock = RwLock(0)

    read_result = await read_during_write()
    assert read_result is None

    async def write_during_read(new_value) -> None:
        nonlocal rwlock
        async with rwlock.read():
            try:
                await asyncio.wait_for(writer(new_value), timeout=0.1)
                raise RuntimeError("Write should block during read")
            except TimeoutError:
                rwlock = RwLock(0)

    await write_during_read(200)

    async def contention_test() -> list[None]:
        ops: list[Any] = []
        for i in range(1000):
            if i % 5 == 0 or i % 19 == 0:
                ops.append(writer(i, yield_while_holding_lock=True))
            elif i % 3 == 0:
                ops.append(reader(yield_while_holding_lock=True))
            else:
                ops.append(reader())
        return await asyncio.gather(*ops)

    contention_results = await contention_test()
    assert len(contention_results) == 1000
    assert all(
        isinstance(res, int) and 0 <= res < 1000
        for res in contention_results
        if res is not None
    )


@pytest.mark.asyncio
async def test_transform_raises_does_not_update() -> None:
    def transform(x: object) -> int:
        val = typing.cast(int, x)
        if val == 2:
            raise ValueError("bad-transform")
        return val * 2

    publisher = Publisher()
    observer = await Observer[int].new_with_transform(publisher, transform)

    # notify a value that causes the transform to raise -> no update
    await publisher.notify(2)
    assert await observer.get() is None

    # notify a valid value -> observer updates
    await publisher.notify(3)
    assert await observer.get() == 6


@pytest.mark.asyncio
async def test_notify_parallel_observers() -> None:
    publisher = Publisher()
    fast_event = asyncio.Event()
    slow_event = asyncio.Event()
    allow_slow_event = asyncio.Event()

    class SlowObserver(IInnerObserverReceiver):
        async def update(self, data: list[Optional[object]]) -> None:
            await allow_slow_event.wait()
            slow_event.set()

    class FastObserver(IInnerObserverReceiver):
        async def update(self, data: list[Optional[object]]) -> None:
            fast_event.set()

    await publisher.add_observer(SlowObserver(), 0)
    await publisher.add_observer(FastObserver(), 0)

    notify_task = asyncio.create_task(publisher.notify("value"))

    await asyncio.wait_for(fast_event.wait(), timeout=0.5)
    assert not slow_event.is_set()

    allow_slow_event.set()
    await notify_task
    assert fast_event.is_set()
    assert slow_event.is_set()


@pytest.mark.asyncio
async def test_notify_parallel_observers_with_exception() -> None:
    publisher = Publisher()
    slow_event = asyncio.Event()

    class SlowObserver(IInnerObserverReceiver):
        async def update(self, data: list[Optional[object]]) -> None:
            slow_event.set()

    class FastObserver(IInnerObserverReceiver):
        async def update(self, data: list[Optional[object]]) -> None:
            raise RuntimeError("fast-failure")

    await publisher.add_observer(SlowObserver(), 0)
    await publisher.add_observer(FastObserver(), 0)

    with pytest.raises(RuntimeError, match="fast-failure"):
        await publisher.notify("value")

    assert slow_event.is_set()
