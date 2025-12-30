from typing import Any, Optional
import asyncio
import typing
import pytest

from n_observer.n_observer import IInnerObserverReceiver, Observer, Publisher


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
async def test_transform_raises_does_not_update(caplog: pytest.LogCaptureFixture) -> None:
    def transform(x: object) -> int:
        val = typing.cast(int, x)
        if val == 2:
            raise ValueError("bad-transform")
        return val * 2

    publisher = Publisher()
    observer = await Observer[int].new_with_transform(publisher, transform)

    # notify a value that causes the transform to raise -> no update
    with caplog.at_level("ERROR"):
        with pytest.raises(ValueError, match="bad-transform"):
            await publisher.notify(2)
    assert any(
        "Observer transform failed." in record.message for record in caplog.records
    )
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
