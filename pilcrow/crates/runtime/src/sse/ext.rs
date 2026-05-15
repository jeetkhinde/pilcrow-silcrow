use crate::sse::{EmitError, SseEmitter};
use futures_core::Stream;
use serde::Serialize;
use tokio_stream::StreamExt as _;

pub trait PilcrowStreamExt: Stream + Sized {
    fn json<'a, T>(
        self,
        target: &'a str,
        emit: &'a SseEmitter,
    ) -> impl std::future::Future<Output = Result<(), EmitError>> + Send + 'a
    where
        Self: Stream<Item = T> + Unpin + Send + 'a,
        T: Serialize + Send + Sync + 'a;
}

impl<S: Stream + Send> PilcrowStreamExt for S {
    async fn json<'a, T>(mut self, target: &'a str, emit: &'a SseEmitter) -> Result<(), EmitError>
    where
        Self: Stream<Item = T> + Unpin + Send + 'a,
        T: Serialize + Send + Sync + 'a,
    {
        while let Some(data) = self.next().await {
            emit.json(target, &data).await?;
        }
        Ok(())
    }
}
