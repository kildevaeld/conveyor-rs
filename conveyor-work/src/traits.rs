use super::package::Package;
use conveyor::futures::prelude::*;
use conveyor::{OneOfFuture, Promise, Result, Station};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Poll, Context};

pub trait ChainStreamer: Stream + Sized
where
    <Self as Stream>::Item: Future,
{
    fn pipe<S: Station>(self, station: S) -> ChainStream<Self, S>;
}

impl<T> ChainStreamer for T
where
    T: Stream,
    <T as Stream>::Item: Future,
{
    fn pipe<S: Station>(self, station: S) -> ChainStream<Self, S> {
        ChainStream {
            stream: self,
            station: Arc::new(station),
        }
    }
}

pub struct ChainStream<Str, St> {
    stream: Str,
    station: Arc<St>,
}

impl<Str, St> Stream for ChainStream<Str, St>
where
    Str: Stream,
    <Str as Stream>::Item: Future<Output = Result<Package>> + 'static + Send,
    St: Station<Input = Package, Output = Package> + 'static + Send + Sync,
{
    type Item = Pin<Box<Future<Output = Result<Package>> + Send>>;

    fn poll_next(self: Pin<&mut Self>, waker: &mut Context) -> Poll<Option<Self::Item>> {
        let this = unsafe { Pin::get_unchecked_mut(self) };

        match unsafe { Pin::new_unchecked(&mut this.stream) }.poll_next(waker) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(ret) => match ret {
                None => Poll::Ready(None),
                Some(m) => {
                    let clone = this.station.clone();
                    Poll::Ready(Some(Box::pin(m.then(move |m| match m {
                        Ok(m) => OneOfFuture::new(Promise::First(clone.execute(m))),
                        Err(e) => OneOfFuture::new(Promise::Second(future::ready(Err(e)))),
                    }))))
                }
            },
        }
    }
}
