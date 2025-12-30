from typing import Any
import asyncio

import pytest

from n_observer.n_observer import RwLock


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
