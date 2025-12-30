use std::{
    any::Any,
    sync::{Arc, Weak},
};

use futures::future::{BoxFuture, join_all};
use tokio::sync::RwLock;

use rs_schema_derive::*;

pub type AnyArc = Arc<dyn Any + Send + Sync>;

#[derive(Debug, Clone)]
pub enum ObserverError {
    TransformError,
}

pub struct Observer<T> {
    inner: Arc<InnerObserverImpl<T>>,
}

struct InnerObserverImpl<T> {
    // arc_swap crate?
    current: RwLock<Option<Arc<T>>>,
    publisher: CachedPublisher,
    transform: Box<dyn Fn(Vec<AnyArc>) -> Result<Arc<T>, ObserverError> + Send + Sync>,
    last_inputs: RwLock<Vec<Option<AnyArc>>>,
    // Keep publisher alive for the lifetime of the observer
    // Mostly desired when observing one or more other observers
    parent_publisher_refs: RwLock<Vec<Arc<dyn Publisher + Send + Sync>>>,
}

#[trait_schema]
pub trait InnerObserverReceiver: Send + Sync {
    fn update(
        &self,
        #[arg(cffi_type = "CffiPointerBuffer<opt_ptr<T>>")] data: Vec<Option<AnyArc>>,
    ) -> BoxFuture<'_, ()>;

    #[func(cffi_impl_no_op)]
    fn hold_strong_publisher_ref(
        &self,
        publisher: Arc<dyn Publisher + Send + Sync>,
    ) -> BoxFuture<'_, ()>;
}

impl<T> InnerObserverImpl<T>
where
    T: Send + Sync + 'static,
{
    async fn get(&self) -> Option<Arc<T>> {
        self.current.read().await.clone()
    }

    // NOTE: `last_inputs` must be updated before calling this
    async fn update_direct(&self, data: AnyArc) {
        *self.current.write().await = Some(data.clone().downcast::<T>().unwrap());
        self.publisher.notify(data).await;
    }

    async fn update_with_transform(&self, data: Vec<Option<AnyArc>>) {
        let inputs = {
            let mut last_inputs = self.last_inputs.write().await;
            data.into_iter()
                .zip(last_inputs.iter_mut())
                // None input implies that publisher didn't notify with an update
                .filter(|(input, _)| input.is_some())
                .for_each(|(input, last_input)| {
                    *last_input = input;
                });
            let mut inputs = Vec::with_capacity(last_inputs.len());
            for input in last_inputs.iter() {
                if let Some(input) = input {
                    inputs.push(input.clone());
                } else {
                    return;
                }
            }
            inputs
        };
        if let Ok(transformed_data) = (self.transform)(inputs) {
            self.update_direct(transformed_data).await;
        }
    }
}

impl<T> InnerObserverReceiver for InnerObserverImpl<T>
where
    T: Send + Sync + 'static,
{
    fn update(&self, data: Vec<Option<AnyArc>>) -> BoxFuture<'_, ()> {
        Box::pin(self.update_with_transform(data))
    }

    fn hold_strong_publisher_ref(
        &self,
        publisher: Arc<dyn Publisher + Send + Sync>,
    ) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            self.parent_publisher_refs.write().await.push(publisher);
        })
    }
}

impl<T> InnerObserverReceiver for Weak<T>
where
    T: InnerObserverReceiver,
{
    fn update(&self, data: Vec<Option<AnyArc>>) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            if let Some(strong) = self.upgrade() {
                strong.update(data).await
            }
        })
    }

    fn hold_strong_publisher_ref(
        &self,
        publisher: Arc<dyn Publisher + Send + Sync>,
    ) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            if let Some(strong) = self.upgrade() {
                strong.hold_strong_publisher_ref(publisher).await;
            }
        })
    }
}

/**
 * Passthrough so Observer can be used as an InnerObserverReceiver.
 */
impl<T> InnerObserverReceiver for Observer<T>
where
    T: Send + Sync + 'static,
{
    fn update(&self, data: Vec<Option<AnyArc>>) -> BoxFuture<'_, ()> {
        self.inner.update(data)
    }

    fn hold_strong_publisher_ref(
        &self,
        publisher: Arc<dyn Publisher + Send + Sync>,
    ) -> BoxFuture<'_, ()> {
        self.inner.hold_strong_publisher_ref(publisher)
    }
}

impl<T> Observer<T>
where
    T: 'static + Send + Sync,
{
    pub async fn new<P>(publisher: &P) -> Self
    where
        P: Publisher + ?Sized,
    {
        let inner = Arc::new(InnerObserverImpl {
            current: RwLock::new(None),
            publisher: CachedPublisher::new(None),
            transform: Box::new(|data: Vec<AnyArc>| Ok(data[0].clone().downcast::<T>().unwrap())),
            last_inputs: RwLock::new(vec![None]),
            parent_publisher_refs: RwLock::new(Vec::new()),
        });

        let initial_value = publisher
            .add_observer(Box::new(Arc::downgrade(&inner)), 0)
            .await;
        inner.update(vec![initial_value]).await;

        Observer { inner }
    }

    pub async fn new_multiparent<'a, I, F>(publishers: I, transform: F) -> Self
    where
        I: IntoIterator<Item = &'a dyn Publisher>,
        F: Fn(Vec<AnyArc>) -> Result<Arc<T>, ObserverError> + Send + Sync + 'static,
    {
        let publishers: Vec<_> = publishers.into_iter().collect();
        let inner = Arc::new(InnerObserverImpl {
            current: RwLock::new(None),
            publisher: CachedPublisher::new(None),
            transform: Box::new(transform),
            last_inputs: RwLock::new(vec![None; publishers.len()]),
            parent_publisher_refs: RwLock::new(Vec::new()),
        });

        let initial_values = join_all(
            publishers
                .into_iter()
                .enumerate()
                .map(|(i, p)| p.add_observer(Box::new(Arc::downgrade(&inner)), i)),
        )
        .await;
        inner.update(initial_values).await;

        Observer { inner }
    }

    pub async fn new_with_transform<P, F>(publisher: &P, transform: F) -> Self
    where
        P: Publisher + ?Sized,
        // TODO: Make transform async
        F: Fn(AnyArc) -> Result<Arc<T>, ObserverError> + Send + Sync + 'static,
    {
        let inner = Arc::new(InnerObserverImpl {
            current: RwLock::new(None),
            publisher: CachedPublisher::new(None),
            transform: Box::new(move |inputs| {
                if let Some(input) = inputs.into_iter().next() {
                    transform(input)
                } else {
                    panic!("Invalid input supplied to transform function");
                }
            }),
            last_inputs: RwLock::new(vec![None]),
            parent_publisher_refs: RwLock::new(Vec::new()),
        });

        let initial_value = publisher
            .add_observer(Box::new(Arc::downgrade(&inner)), 0)
            .await;
        if let Some(data) = initial_value {
            inner.update_with_transform(vec![Some(data)]).await;
        }

        Observer { inner }
    }
}

#[trait_schema(T = "ptr<void>")]
pub trait Observable<T>: Publisher + InnerObserverReceiver {
    fn get(&self) -> BoxFuture<'_, Option<Arc<T>>>;
}

impl<T> Observable<T> for Observer<T>
where
    T: Send + Sync + 'static,
{
    fn get(&self) -> BoxFuture<'_, Option<Arc<T>>> {
        Box::pin(self.inner.get())
    }
}

#[trait_schema]
pub trait Publisher {
    // TODO: Does this output really need to be optional?
    fn add_observer(
        &self,
        #[arg(cffi_type = "CffiInnerObserverReceiver")] observer: Box<dyn InnerObserverReceiver>,
        input_index: usize,
    ) -> BoxFuture<'_, Option<AnyArc>>;

    fn notify(&self, data: AnyArc) -> BoxFuture<'_, ()>;
}

pub struct CachedPublisher {
    // [(input_index, observer)]
    observers: RwLock<Vec<(usize, Box<dyn InnerObserverReceiver>)>>,
    current: RwLock<Option<AnyArc>>,
}

impl CachedPublisher {
    pub fn new(initial_value: Option<AnyArc>) -> Self {
        Self {
            observers: RwLock::new(Vec::new()),
            current: RwLock::new(initial_value),
        }
    }

    async fn add_observer_impl(
        &self,
        observer: Box<dyn InnerObserverReceiver>,
        input_index: usize,
    ) -> Option<AnyArc> {
        let mut observers = self.observers.write().await;
        observers.push((input_index, observer));

        self.current.read().await.clone()
    }

    async fn notify_impl(&self, data: AnyArc) {
        *self.current.write().await = Some(data.clone());
        let observers = self.observers.read().await;
        join_all(observers.iter().map(|(i, o)| {
            let mut inputs = vec![None; *i + 1];
            inputs[*i] = Some(data.clone());
            o.update(inputs)
        }))
        .await;
    }
}

impl Publisher for CachedPublisher {
    fn add_observer(
        &self,
        observer: Box<dyn InnerObserverReceiver>,
        input_index: usize,
    ) -> BoxFuture<'_, Option<AnyArc>> {
        Box::pin(self.add_observer_impl(observer, input_index))
    }

    fn notify<'a>(&'a self, data: AnyArc) -> BoxFuture<'a, ()> {
        Box::pin(self.notify_impl(data))
    }
}

impl<T> Publisher for Observer<T>
where
    T: Send + Sync + 'static,
{
    fn add_observer(
        &self,
        observer: Box<dyn InnerObserverReceiver>,
        input_index: usize,
    ) -> BoxFuture<'_, Option<AnyArc>> {
        Box::pin(async move {
            // When an observer is being used as a publisher,
            // we assume that it should be kept alive for the lifetime the
            // child `observer`. This allows for patterns like:
            // ```
            // fn get_observer2() -> Observer<T> {
            //     let observer1 = get_observer1();
            //     let observer2 = Observer::new_with_transform(&observer1, |data| {
            //         // Transform the data in some way
            //         data
            //     }).await;

            //     observer2
            //     // observer1 dropped here, but its InnerObserverReceiver
            //     // is still held by observer2, so it will not be dropped.
            // }
            observer.hold_strong_publisher_ref(self.inner.clone()).await;
            self.inner.add_observer(observer, input_index).await
        })
    }

    fn notify<'a>(&'a self, data: AnyArc) -> BoxFuture<'a, ()> {
        self.inner.notify(data)
    }
}

impl<T> Publisher for InnerObserverImpl<T>
where
    T: Send + Sync,
{
    fn add_observer(
        &self,
        observer: Box<dyn InnerObserverReceiver>,
        input_index: usize,
    ) -> BoxFuture<'_, Option<AnyArc>> {
        self.publisher.add_observer(observer, input_index)
    }

    fn notify<'a>(&'a self, data: AnyArc) -> BoxFuture<'a, ()> {
        self.publisher.notify(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_observer_single() {
        let publisher = CachedPublisher::new(None);
        let observer = Observer::new(&publisher).await;

        let data = Arc::new("Hello".to_string());
        publisher.notify(data.clone()).await;

        assert_eq!(observer.get().await, Some(data));
    }

    #[tokio::test]
    async fn test_observer_single_multiple_pub() {
        let publisher = CachedPublisher::new(None);
        let observer = Observer::new(&publisher).await;

        let data = Arc::new("Hello".to_string());
        publisher.notify(data.clone()).await;

        assert_eq!(observer.get().await, Some(data));

        let data2 = Arc::new("World".to_string());
        publisher.notify(data2.clone()).await;

        assert_eq!(observer.get().await, Some(data2));
    }

    #[tokio::test]
    async fn test_observer_multiple() {
        let publisher = CachedPublisher::new(None);
        let observer1 = Observer::new(&publisher).await;
        let observer2 = Observer::new(&publisher).await;

        let data = Arc::new("Hello".to_string());
        publisher.notify(data.clone()).await;

        assert_eq!(observer1.get().await, Some(data.clone()));
        assert_eq!(observer2.get().await, Some(data));
    }

    #[tokio::test]
    async fn test_observer_chained() {
        let publisher = CachedPublisher::new(None);
        let observer1 = Observer::new(&publisher).await;
        let observer2 = Observer::new(&observer1).await;

        let data = Arc::new("Hello".to_string());
        publisher.notify(data.clone()).await;

        assert_eq!(observer1.get().await, Some(data.clone()));
        assert_eq!(observer2.get().await, Some(data));
    }

    #[tokio::test]
    async fn test_observer_multiple_publishers() {
        let publisher1 = CachedPublisher::new(None);
        let publisher2 = CachedPublisher::new(None);
        let publishers: Vec<&dyn Publisher> = vec![&publisher1, &publisher2];
        let observer = Observer::new_multiparent(publishers, |data| {
            let data1 = data[0].clone().downcast::<String>().unwrap().len();
            let data2 = data[1].clone().downcast::<String>().unwrap().len();
            Ok(Arc::new(data1 + data2))
        })
        .await;

        let data1 = Arc::new("Hello".to_string());
        let data2 = Arc::new("World!".to_string());

        publisher1.notify(data1.clone()).await;
        publisher2.notify(data2.clone()).await;

        assert_eq!(observer.get().await, Some(Arc::new(11)));

        // Notify again with different data
        let data3 = Arc::new("Centconf".to_string());
        publisher1.notify(data3.clone()).await;

        assert_eq!(observer.get().await, Some(Arc::new(14)));
    }

    #[tokio::test]
    async fn test_observer_with_transform() {
        let publisher = CachedPublisher::new(None);
        let observer = Observer::new_with_transform(&publisher, |data| {
            Ok(Arc::new(data.downcast::<String>().unwrap().len()))
        })
        .await;

        let data = Arc::new("Hello".to_string());
        publisher.notify(data.clone()).await;

        assert_eq!(observer.get().await, Some(Arc::new(5)));
    }

    #[tokio::test]
    async fn test_observer_chain_middle_dropped() {
        let publisher = CachedPublisher::new(None);
        let observer1 = Observer::new_with_transform(&publisher, |data| {
            Ok(Arc::new(data.downcast::<String>().unwrap().len()))
        })
        .await;

        let observer2 = Observer::new_with_transform(&observer1, |data| {
            Ok(Arc::new(*data.downcast::<usize>().unwrap() * 2))
        })
        .await;

        let data = Arc::new("Hello".to_string());
        publisher.notify(data.clone()).await;

        assert_eq!(observer2.get().await, Some(Arc::new(10)));

        // Drop observer1, but observer2 should still work
        drop(observer1);

        let data2 = Arc::new("World!".to_string());
        publisher.notify(data2.clone()).await;

        assert_eq!(observer2.get().await, Some(Arc::new(12)));
    }

    #[tokio::test]
    async fn test_observer_initial_value() {
        let publisher = CachedPublisher::new(Some(Arc::new("Init".to_string())));
        let observer = Observer::new(&publisher).await;

        // Observer should pick up the initial cached value without an explicit notify
        assert_eq!(observer.get().await, Some(Arc::new("Init".to_string())));

        // After notifying with a new value, observer should update
        let new = Arc::new("New".to_string());
        publisher.notify(new.clone()).await;
        assert_eq!(observer.get().await, Some(new));
    }

    #[tokio::test]
    async fn test_observer_transform_error_single() {
        let publisher = CachedPublisher::new(None);

        // Transform returns Err for short strings, Ok for longer ones
        let observer = Observer::new_with_transform(&publisher, |data| {
            let s = data.downcast::<String>().unwrap();
            if s.len() < 10 {
                Err(ObserverError::TransformError)
            } else {
                Ok(Arc::new(s.len()))
            }
        })
        .await;

        // Short string -> transform returns Err -> observer should not update
        let short = Arc::new("short".to_string());
        publisher.notify(short.clone()).await;
        assert_eq!(observer.get().await, None);

        // Long string -> transform returns Ok -> observer should update
        let long = Arc::new("a very long string".to_string());
        publisher.notify(long.clone()).await;
        assert_eq!(observer.get().await, Some(Arc::new(18usize)));
    }

    #[tokio::test]
    async fn test_observer_transform_error_multiparent() {
        let publisher1 = CachedPublisher::new(None);
        let publisher2 = CachedPublisher::new(None);
        let publishers: Vec<&dyn Publisher> = vec![&publisher1, &publisher2];

        // Transform always returns Err
        let observer = Observer::<usize>::new_multiparent(publishers, |_data| {
            Err(ObserverError::TransformError)
        })
        .await;

        // Notify both publishers; since transform errors, observer must not update
        let d1 = Arc::new("Hello".to_string());
        let d2 = Arc::new("World".to_string());
        publisher1.notify(d1.clone()).await;
        publisher2.notify(d2.clone()).await;

        assert_eq!(observer.get().await, None);
    }
}
