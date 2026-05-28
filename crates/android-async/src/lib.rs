use futures::Stream;
use tokio::sync::{mpsc, oneshot};

/// Wraps a one-shot Java callback into a `Future`.
///
/// # Example
/// ```rust,ignore
/// let value = JavaCallback::new(|tx| {
///     // Call some Java method that will eventually call tx.send(result)
///     some_java_call_with_callback(tx);
/// }).await;
/// ```
pub struct JavaCallback<T: Send + 'static> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: Send + 'static> JavaCallback<T> {
    /// Creates a future that resolves when the callback sends a value.
    ///
    /// `setup` receives the sender end of a oneshot channel.  Call `tx.send(value)`
    /// from within the Java callback (e.g. via JNI) to resolve the future.
    pub fn new<F>(setup: F) -> impl std::future::Future<Output = T>
    where
        F: FnOnce(oneshot::Sender<T>),
    {
        let (tx, rx) = oneshot::channel();
        setup(tx);
        async move {
            rx.await
                .expect("JavaCallback: sender dropped without sending a value")
        }
    }
}

/// Wraps a multi-shot Java callback / event source into a `Stream`.
///
/// # Example
/// ```rust,ignore
/// let stream = JavaStream::new(|tx| {
///     // Register a listener that calls tx.send(event) on each event
///     register_java_listener(tx);
/// });
/// ```
pub struct JavaStream<T: Send + 'static> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: Send + 'static> JavaStream<T> {
    /// Creates a stream fed by an unbounded mpsc channel.
    ///
    /// `setup` receives the sender end.  Drop all senders to end the stream.
    pub fn new<F>(setup: F) -> impl Stream<Item = T>
    where
        F: FnOnce(mpsc::UnboundedSender<T>),
    {
        let (tx, rx) = mpsc::unbounded_channel();
        setup(tx);
        futures::stream::unfold(rx, |mut rx| async move {
            let item = rx.recv().await?;
            Some((item, rx))
        })
    }
}
