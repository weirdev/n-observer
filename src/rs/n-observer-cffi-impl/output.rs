use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct CffiInnerObserverReceiver {
    pub update_fut: extern "C" fn(*const c_void, *const c_void) -> *const c_void,
    pub hold_strong_publisher_ref_fut: extern "C" fn(*const c_void, *const c_void) -> *const c_void,
}
