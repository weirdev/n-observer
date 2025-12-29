# Observer CFFI Implementation
Logic for CFFI types for the observer rust library.

Maintained as a separate crate so these can be shared as a rust library without exposing the CFFI bindings themselves.

### CffiInnerObserverReceiver
```
pub struct CffiInnerObserverReceiver {
    // ptr<&dyn InnerObserverReceiver>
    pub self_ptr: *const c_void,
    // self_ptr -> CffiPointerBuffer<opt_ptr<T>> -> CffiFuture<ptr<T>>
    pub update_fut: extern "C" fn(*const c_void, CffiPointerBuffer) -> *const c_void,
    // self_ptr -> ptr<&dyn Publisher> -> CffiFuture<ptr<T>>
    // TODO: Second arg should be CffiPublisher
    pub hold_strong_publisher_ref_fut: extern "C" fn(*const c_void, *const c_void) -> *const c_void,
}
```

### CffiPublisher
```
pub struct CffiPublisher {
    pub self_ptr: *const c_void,
    // self_ptr -> CffiInnerObserverReceiver -> usize -> CffiFuture<ptr<opt_ptr<T>>>
    pub add_observer_fut:
        extern "C" fn(*const c_void, CffiInnerObserverReceiver, c_ulong) -> *const c_void,
    // self_ptr -> ptr<T> -> CffiFuture<ptr<1>>
    pub notify_fut: extern "C" fn(*const c_void, *const c_void) -> *const c_void,
}
```

## Regenerating and validating generated bindings
The build script regenerates both the Rust and Python CFFI bindings. Run
`cargo build -p observer_cffi_helpers` from `src/rust` to refresh the generated
files (including `src/py/centconf/observer_cffi/ior_cffi_traits.py`).

For a quick equivalence check against the checked-in Python output without
manually diffing, execute `scripts/compare_async_cffi_output.sh` from the repo
root. The script snapshots the current file, rebuilds the bindings, and shows a
diff so you can confirm the regenerated output matches what is committed.
