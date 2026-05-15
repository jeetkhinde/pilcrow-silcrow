// ./src/sse/interval.rs
use std::time::Duration;
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::{Stream, StreamExt};

pub fn interval(duration: Duration) -> impl Stream<Item = ()> + Send + 'static {
    let interval = tokio::time::interval(duration);
    IntervalStream::new(interval).map(|_| ())
}
