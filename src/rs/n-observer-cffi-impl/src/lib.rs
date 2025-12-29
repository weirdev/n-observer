mod ior_cffi_traits;
mod observable_cffi_traits;
mod publisher_cffi_traits;

use std::{ffi::c_void, sync::Arc};

use async_cffi::{CffiFuture, SafePtr};

use n_observer::{CachedPublisher, Observable, Observer, Publisher};

pub use ior_cffi_traits::CffiInnerObserverReceiver;

pub use publisher_cffi_traits::CffiPublisher;

pub use observable_cffi_traits::CffiObservable;

pub async fn observer_new_async<P>(publisher: P) -> CffiObservable
where
    P: Publisher + Send + Sync + 'static,
{
    let observer: Box<dyn Observable<SafePtr>> = Box::new(Observer::new(&publisher).await);
    let cffi = (*observer).into();
    Box::leak(observer); // Leak to keep alive
    cffi
}

/// `observer_get_ptr(observer: CffiObservable) -> opt_ptr<T>`
pub async fn observer_get_ptr(observer: CffiObservable) -> *const c_void {
    let value = observer.get().await;
    value.map_or(std::ptr::null(), |v| {
        let ptr = (*v).0;
        // TODO: Leaking this to ensure the underlying data lives long enough,
        // need to free it later.
        let _ = Arc::into_raw(v);
        ptr
    })
}

/// `int_publisher_new(initial_value: i32) -> CffiPublisher`
pub fn int_publisher_new(initial_value: i32) -> CffiPublisher {
    let initial_value = Box::new(initial_value);
    let initial_value = Box::into_raw(initial_value);
    let initial_value = initial_value as *const c_void;
    let publisher: Box<dyn Publisher> =
        Box::new(CachedPublisher::new(Some(Arc::new(SafePtr(initial_value)))));

    let cffi_publisher = (*publisher).into();
    Box::leak(publisher); // Leak to keep alive
    cffi_publisher
}

/// `observer_new_fut(publisher: CffiPublisher) ->
///   ptr<CffiFuture<ptr<CffiObservable>>>`
pub fn observer_new_fut(publisher: CffiPublisher) -> *mut c_void {
    let observer_fut = observer_new_async(publisher);
    let observer_fut = CffiFuture::from_rust_future_boxed(observer_fut);
    observer_fut.into_raw()
}

/// `observer_get_fut(observer: CffiObservable) -> ptr<CffiFuture<ptr<opt_ptr<T>>>>`
pub fn observer_get_fut(observer: CffiObservable) -> *mut c_void {
    let fut = observer_get_ptr(observer);
    let fut = CffiFuture::from_rust_future_boxed(fut);
    fut.into_raw()
}
