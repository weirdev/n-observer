# N-Observer Copilot Instructions

## Project Overview
N-Observer is a dual-language async/await observer pattern implementation supporting:
- **Single-parent observers**: One publisher → one observer with optional transform
- **Multi-parent observers**: Multiple publishers → one observer combining inputs
- **Observer chains**: Observers can be chained as both publishers and observers

Key characteristic: Observers **hold strong references** to parent publishers, preventing premature garbage collection.

## Architecture

### Core Abstraction (Both Languages)
Three interfaces define the pub/sub system:
- `IPublisher`: Can add observers and notify all with `add_observer()` and `notify()`
- `IInnerObserverReceiver`: Internal interface receiving batch updates via `update()`
- `IObservable`: Combines both interfaces; represents a value holder that's both publisher and subscriber

### Rust Implementation (`src/rs/n-observer/`)
- **observer.rs**: Core `Observer<T>`, `Publisher`, traits using generic `AnyArc` (Arc<dyn Any + Send + Sync>)
- Uses `tokio::sync::RwLock` for async concurrency
- Transforms can fail with `ObserverError::TransformError` to skip updates
- **CFFI bridge**: `n-observer-cffi-impl/` generates bindings via `async-cffi` crate for Python interop

### Python Implementation (`src/py/n_observer/`)
- **core.py**: Pure Python async implementation using `async_rwlock.RwLock`
- Mirrors Rust API but uses `typing` generics (Python 3.9 compatible)
- `ObserverError` exception allows transforms to skip updates
- Python tests import from Rust-built CFFI library when integration testing

## Build & Test Workflow

### Commands
- **Full test suite**: `python run_tests.py` (root) — builds Rust, runs Rust+Python tests
- **Rust only**: `cargo build --tests --lib` + `cargo test` (src/rs/)
- **Python only**: `python -m pytest -v` (src/py/, requires PYTHONPATH set)

### CFFI Binding Updates
**After modifying Rust trait signatures** (marked with `#[trait_schema]`):
1. Run: `cargo build -p observer_cffi_helpers` (src/rs/)
2. Bindings regenerate in:
   - `src/rs/n-observer-cffi-impl/output.rs` (Rust)
   - `src/py/n_observer/observer_cffi/ior_cffi_traits.py` (Python)
3. Verify: `scripts/compare_async_cffi_output.sh` from repo root

## Key Patterns

### Single-Parent Observer (Most Common)
```python
# Rust
let observer = Observer::new(publisher, |x| transform(x)).await?;

# Python
observer = await Observer[int].new_with_transform(publisher, lambda x: x * 2)
```

### Multi-Parent Observer
```python
# Combines multiple inputs; transform called only when ALL parents have values
observer = await Observer[int].new_multiparent(
    [pub1, pub2],
    lambda inputs: len(inputs[0]) + len(inputs[1])
)
```

### Transform Error Handling
- Raise `ObserverError` in transform to skip update (no notify downstream)
- Other exceptions propagate and are logged but still trigger skip behavior
- Always inspect logs for silently skipped transforms

### Observer Chains
```python
obs1 = await Observer[int].new_with_transform(pub, lambda x: x + 1)
obs2 = await Observer[str].new(obs1)  # obs1 acts as publisher for obs2
```

## Developer Conventions

### Type Safety
- **Rust**: Leverage generics; transforms must preserve type safety via `Arc<dyn Any>`
- **Python**: Use `typing.cast()` in transforms; mark `Observer[T]` at creation for IDE support

### Testing
- Async tests use `pytest.mark.asyncio` decorator (Python)
- No parametrized tests yet; follow pattern in test_observer.py
- Tests cover: single/multi-parent, chaining, transform errors, multiple notifications

### Documentation
- Keep docstrings brief; explain *why* not *what* for non-obvious methods
- RwLock.read() yields shared reference; treat as immutable unless holding write lock

## Common Tasks

### Adding a New Observer Variant
1. Add method to `IObservable` trait (Rust: observer.rs, Python: core.py)
2. Update `InnerObserverImpl` constructor if needed
3. Write tests in parallel (src/rs/tests/ or src/py/tests/)
4. If modifying traits, regenerate CFFI: `cargo build -p observer_cffi_helpers`

### Debugging Async Issues
- Use `tokio::test` macro (Rust); `pytest-asyncio` (Python)
- Check RwLock contention; observers hold write locks during transform
- Verify publisher lifetime: strong refs prevent drops

### Adding Dependencies
- **Rust**: Update Cargo.toml in respective crate; test with `cargo build`
- **Python**: Update src/py/requirements.txt; test with `python -m pytest`

## Cross-Language Interop Notes
- Python tests load compiled Rust library via CFFI bindings
- Type translation happens in `ior_cffi_traits.py` auto-generated file
- Rust `AnyArc` ↔ Python `object` mapping is transparent but can mask type errors
