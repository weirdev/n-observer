# N-Observer: Copilot / Agent Guidance

**Purpose:** Quickly orient an AI coding agent to the repo design, workflows, and touchpoints an engineer will need.

**Architecture (big picture)**
- **Core idea:** async observer pattern implemented in Rust (core) and mirrored in Python (pure-Python implementation + CFFI bridge).
- **Rust source:** [src/rs/n-observer/src/observer.rs](src/rs/n-observer/src/observer.rs) — main traits, `Observer<T>`, `Publisher` and `AnyArc` usage.
- **Python core:** [src/py/n_observer/src/n_observer/core.py](src/py/n_observer/src/n_observer/core.py) — async implementation, `ObserverError`, and RwLock usage.
- **CFFI bridge:** [src/rs/n-observer-cffi-impl](src/rs/n-observer-cffi-impl) generates bindings consumed by [src/py/n_observer/observer_cffi](src/py/n_observer/observer_cffi).

**Critical workflows & commands**
- **Run full test/build:** `python run_tests.py` (project root) — builds Rust, generates CFFI, runs Python tests.
- **Rebuild CFFI bindings:** `cargo build -p observer_cffi_helpers` (in `src/rs/`) — regenerates `output.rs` and Python trait files.
- **Rust tests:** run `cargo test` inside `src/rs/` for unit-level checks.
- **Python tests:** run `python -m pytest -v` in `src/py/` (ensure `PYTHONPATH` includes built CFFI module when doing integration tests).

**Project-specific conventions**
- **Observers hold strong refs:** Publishers keep strong references to parents; don't rely on GC for lifecycle cleanup.
- **Transforms can skip updates:** Raising `ObserverError` (Python) or returning `TransformError` (Rust) is the intended way to skip downstream notifications.
- **Locks & transforms:** Transforms execute while locks are held—watch for `tokio::sync::RwLock` contention in Rust and `async_rwlock.RwLock` in Python.

**Integration points to watch**
- Generated files: [src/rs/n-observer-cffi-impl/output.rs](src/rs/n-observer-cffi-impl/output.rs) and [src/py/n_observer/observer_cffi/ior_cffi_traits.py](src/py/n_observer/observer_cffi/ior_cffi_traits.py).
- Tests referencing integration behavior: [src/py/tests/n_observer/test_observer.py](src/py/tests/n_observer/test_observer.py).

**What to change carefully**
- Trait signatures in Rust annotated with `#[trait_schema]`: changing these requires regenerating CFFI and updating Python trait shims.
- Any change to observer lifetime, locking, or transform semantics must include tests in both Rust and Python where relevant.

**Quick examples & pointers**
- New single-parent observer (Python): follow the constructor pattern in `core.py` and mirror tests in `src/py/tests/n_observer/test_observer.py`.
- Regenerate bindings after trait changes: run `cargo build -p observer_cffi_helpers` then `python run_tests.py`.

If you'd like this shortened, expanded for a specific agent role, or committed to `.github/copilot-instructions.md`, tell me which and I'll proceed.