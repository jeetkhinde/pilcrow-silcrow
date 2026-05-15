// ./src/sse/watch.rs

//use std::pin::Pin;
use tokio::sync::watch;
use tokio_stream::Stream;
use tokio_stream::wrappers::WatchStream as TokioWatchStream;

pub fn watch<T: Clone + Send + Sync + 'static>(
    rx: watch::Receiver<T>,
) -> impl Stream<Item = T> + Send + 'static {
    TokioWatchStream::new(rx)
}
