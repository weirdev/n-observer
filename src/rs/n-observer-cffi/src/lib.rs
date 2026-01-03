use std::ffi::c_void;

use n_observer_cffi_impl::{CffiObservable, CffiPublisher};

/// `int_publisher_new(initial_value: i32) -> CffiPublisher`
#[unsafe(no_mangle)]
pub extern "C" fn int_publisher_new(initial_value: i32) -> CffiPublisher {
    n_observer_cffi_impl::int_publisher_new(initial_value)
}

/// `observer_new_fut(publisher: CffiPublisher) ->
///   ptr<CffiFuture<ptr<CffiObservable>>>`
#[unsafe(no_mangle)]
pub extern "C" fn observer_new_fut(publisher: CffiPublisher) -> *mut c_void {
    n_observer_cffi_impl::observer_new_fut(publisher)
}

/// `observer_get_fut(observer: CffiObservable) -> ptr<CffiFuture<ptr<opt_ptr<T>>>>`
#[unsafe(no_mangle)]
pub extern "C" fn observer_get_fut(observer: CffiObservable) -> *mut c_void {
    n_observer_cffi_impl::observer_get_fut(observer)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use async_cffi::{
        CffiFuture, CffiPointerBuffer, SafePtr, box_i32, poll_cffi_future, waker_wrapper_new,
    };

    use n_observer::{CachedPublisher, InnerObserverReceiver, Observable, Observer, Publisher};
    use n_observer_cffi_impl::{CffiInnerObserverReceiver, CffiObservable};
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_observer_fut() {
        let val = Arc::<SafePtr>::new(SafePtr(box_i32(5)));
        let publisher = Box::new(CachedPublisher::new(Some(val.clone())));
        let publisher: &dyn Publisher = &*publisher;
        let publisher = publisher.into();
        let observer_fut_ptr = observer_new_fut(publisher);
        assert!(!observer_fut_ptr.is_null());

        extern "C" fn dummy_waker() {}
        let waker_ptr = waker_wrapper_new(dummy_waker);
        assert!(!waker_ptr.is_null());

        let observer_out = poll_cffi_future(observer_fut_ptr, waker_ptr);
        assert!(!observer_out.is_null());
        let observer = unsafe {
            (observer_out as *mut CffiObservable)
                .as_mut()
                .expect("Observer box cannot be null")
        };
        let value: Option<Arc<SafePtr>> = observer.get().await;
        assert!(value.is_some());
        let value = value.unwrap();
        let value = value.0 as *const i32;
        let value = unsafe { &*value };

        assert_eq!(*value, 5);
    }

    #[tokio::test]
    #[serial]
    async fn test_observer_new_from_observer() {
        let publisher_ptr = int_publisher_new(10);
        let observer_ptr_fut = observer_new_fut(publisher_ptr.clone());
        let observer_ptr_fut = unsafe {
            (observer_ptr_fut as *mut CffiFuture)
                .as_mut()
                .expect("Observer future cannot be null")
        };
        let observer_ptr = observer_ptr_fut.await;
        let observer = unsafe {
            (observer_ptr as *mut CffiObservable)
                .as_mut()
                .expect("Observer box cannot be null")
        };

        let observer_publisher_ptr = observer.cffi_publisher.clone();
        let new_observer_ptr = observer_new_fut(observer_publisher_ptr);
        assert!(!new_observer_ptr.is_null());

        let new_observer_ptr_fut = unsafe {
            (new_observer_ptr as *mut CffiFuture)
                .as_mut()
                .expect("New observer future cannot be null")
        };
        let new_observer_ptr = new_observer_ptr_fut.await;
        assert!(!new_observer_ptr.is_null());
        let new_observer = unsafe {
            (new_observer_ptr as *mut CffiObservable)
                .as_mut()
                .expect("New observer box cannot be null")
        };

        let new_observer_value_ptr_ptr_fut = observer_get_fut(new_observer.clone());
        let new_observer_value_ptr_ptr_fut = unsafe {
            (new_observer_value_ptr_ptr_fut as *mut CffiFuture)
                .as_mut()
                .expect("New observer value future cannot be null")
        };
        let new_observer_value_ptr_ptr = new_observer_value_ptr_ptr_fut.await;
        assert!(!new_observer_value_ptr_ptr.is_null());

        let new_observer_value = unsafe { &**(new_observer_value_ptr_ptr as *const Box<i32>) };
        assert_eq!(*new_observer_value, 10);

        // Notify the original observer to change its value
        let new_value_ptr = box_i32(30);
        publisher_ptr.notify(Arc::new(SafePtr(new_value_ptr))).await;

        // Check that the new observer now reflects the updated value
        let updated_new_observer_value_ptr_ptr_fut = observer_get_fut(new_observer.clone());
        let updated_new_observer_value_ptr_ptr_fut = unsafe {
            (updated_new_observer_value_ptr_ptr_fut as *mut CffiFuture)
                .as_mut()
                .expect("Updated new observer value future cannot be null")
        };
        let updated_new_observer_value_ptr_ptr = updated_new_observer_value_ptr_ptr_fut.await;
        assert!(!updated_new_observer_value_ptr_ptr.is_null());
        let updated_new_observer_value =
            unsafe { &**(updated_new_observer_value_ptr_ptr as *const Box<i32>) };
        assert_eq!(*updated_new_observer_value, 30);
    }

    #[tokio::test]
    #[serial]
    async fn test_cffi_publisher_to_py_observer() {
        println!("Testing CffiPublisher with CffiObserver...");
        let publisher_ptr = int_publisher_new(100);
        println!("Publisher pointer created: {:?}", publisher_ptr);

        // Native Observer (rust here, but represents any language's native observer)
        let native_observer = Observer::<SafePtr>::new(&publisher_ptr).await;
        println!("Native observer created");

        let result = native_observer.get().await;
        println!("Native observer get called");
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(unsafe { *(result.0 as *const i32) }, 100);

        // Notify the publisher to change its value
        let new_value_ptr = box_i32(200);
        // WIP: Problem probably next line
        publisher_ptr.notify(Arc::new(SafePtr(new_value_ptr))).await;
        println!("Notified with new value");

        // Check that the observer now reflects the updated value
        let updated_result = native_observer.get().await;
        println!("Observer.get() #2");
        assert!(updated_result.is_some());
        let updated_result = updated_result.unwrap();
        assert_eq!(unsafe { *(updated_result.0 as *const i32) }, 200);

        // Chain observers
        let deref_observer = Observer::new_with_transform(&native_observer, |value| {
            let value = value.downcast::<SafePtr>().expect("Value must be SafePtr");
            let int_value = unsafe { *(value.0 as *const i32) };
            Ok(Arc::new(int_value))
        })
        .await;

        let deref_observer_value = deref_observer.get().await;
        assert!(deref_observer_value.is_some());
        let deref_observer_value = deref_observer_value.unwrap();
        assert_eq!(*deref_observer_value, 200);

        // Notify the original publisher to change its value again
        let new_value_ptr = box_i32(300);
        publisher_ptr.notify(Arc::new(SafePtr(new_value_ptr))).await;

        // Check that the observer now reflects the updated value
        let updated_result = deref_observer.get().await;
        assert!(updated_result.is_some());
        assert_eq!(*updated_result.unwrap(), 300);
    }

    #[tokio::test]
    #[serial]
    async fn test_py_observer_empty_update() {
        println!("Testing CffiPublisher with CffiObserver...");
        let publisher = int_publisher_new(100);
        println!("Publisher pointer created: {:?}", publisher);

        // Native Observer (rust here, but represents any language's native observer)
        let native_observer = Box::new(Observer::<SafePtr>::new(&publisher).await);
        println!("Native observer created");

        let result = native_observer.get().await;
        println!("Native observer get called");
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(unsafe { *(result.0 as *const i32) }, 100);

        let dyn_observer: &dyn InnerObserverReceiver = &*native_observer;
        let cffi_observer: CffiInnerObserverReceiver = dyn_observer.into();

        // Send an empty update
        let fut_ptr =
            (cffi_observer.update_fut)(cffi_observer.self_ptr, CffiPointerBuffer::new_empty());
        let fut = unsafe {
            (fut_ptr as *mut CffiFuture)
                .as_mut()
                .expect("CffiFuture cannot be null")
        };
        fut.await;

        // Check that the native observer's value is unchanged
        let result = native_observer.get().await;
        println!("Native observer get called");
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(unsafe { *(result.0 as *const i32) }, 100);
    }

    #[tokio::test]
    #[serial]
    async fn test_py_multi_input_observer() {
        println!("Testing CffiPublisher with multi-input CffiObserver...");
        let publisher1_ptr = int_publisher_new(10);
        let publisher2_ptr = int_publisher_new(20);

        // Native Observer (rust here, but represents any language's native observer)
        let native_observer = Observer::<SafePtr>::new_multiparent(
            vec![
                &publisher1_ptr as &dyn Publisher,
                &publisher2_ptr as &dyn Publisher,
            ],
            |values| {
                let sum = values.into_iter().fold(0, |acc, val| {
                    let value = val.downcast::<SafePtr>().expect("Value must be SafePtr");
                    let value = unsafe { *(value.0 as *const i32) };
                    acc + value
                });
                Ok(Arc::new(SafePtr(box_i32(sum))))
            },
        )
        .await;
        println!("Native multi-input observer created");

        let result = native_observer.get().await;
        println!("Native multi-input observer get called");
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(unsafe { *(result.0 as *const i32) }, 10 + 20);

        // Notify the first publisher to change its value
        let new_value1_ptr = box_i32(30);
        publisher1_ptr
            .notify(Arc::new(SafePtr(new_value1_ptr)))
            .await;
        println!("Notified publisher 1 with new value");

        // Check that the observer now reflects the updated value
        let updated_result = native_observer.get().await;
        println!("Native multi-input observer get called #2");
        assert!(updated_result.is_some());
        let updated_result = updated_result.unwrap();
        assert_eq!(unsafe { *(updated_result.0 as *const i32) }, 30 + 20);
    }
}
