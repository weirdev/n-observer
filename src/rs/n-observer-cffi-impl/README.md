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
This crate's build script regenerates the Rust CFFI trait files in
`src/rs/n-observer-cffi-impl/src/`.

From the Rust workspace root, run:

`cargo build -p n-observer-cffi-impl`

from `src/rs`.

To validate the checked-in generated files, diff the generated trait sources
after the build:

`git diff -- src/rs/n-observer-cffi-impl/src/*_cffi_traits.rs`

If the diff is empty, the generated output in the repository is up to date.
