use super::package::Package;
use conveyor::futures::prelude::*;
use conveyor::{ConveyorError, Result, Station};
use std::pin::Pin;
use std::task::{Poll, Waker};

pub type BoxedStation = Box<
    Station<
            Input = Package,
            Output = Package,
            Future = Pin<Box<Future<Output = Result<Package>> + Send>>,
        > + Send
        + Sync,
>;

pub struct BoxWrap {
    inner: Box<
        Station<
                Input = Package,
                Output = Package,
                Future = Pin<Box<Future<Output = Result<Package>> + Send>>,
            > + Send
            + Sync,
    >,
}

impl BoxWrap {
    pub fn new(
        station: Box<
            Station<
                    Input = Package,
                    Output = Package,
                    Future = Pin<Box<Future<Output = Result<Package>> + Send>>,
                > + Send
                + Sync,
        >,
    ) -> BoxWrap {
        BoxWrap { inner: station }
    }
}

impl Station for BoxWrap {
    type Input = Package;
    type Output = Package;
    type Future = Pin<Box<Future<Output = Result<Package>> + Send>>;
    fn execute(&self, input: Self::Input) -> Self::Future {
        self.inner.execute(input)
    }
}

pub enum FutureOrErr<F, V>
where
    F: Future<Output = Result<V>>,
{
    Err(Option<ConveyorError>),
    Future(F),
}

pub struct FutureOrErrFuture<F, V>
where
    F: Future<Output = Result<V>>,
{
    inner: FutureOrErr<F, V>,
}

impl<F, V> From<F> for FutureOrErrFuture<F, V>
where
    F: Future<Output = Result<V>>,
{
    fn from(future: F) -> FutureOrErrFuture<F, V> {
        FutureOrErrFuture {
            inner: FutureOrErr::Future(future),
        }
    }
}

impl<F, V> FutureOrErrFuture<F, V>
where
    F: Future<Output = Result<V>>,
{
    pub fn from_err(future: ConveyorError) -> FutureOrErrFuture<F, V> {
        FutureOrErrFuture {
            inner: FutureOrErr::Err(Some(future)),
        }
    }
}

impl<F, V> Future for FutureOrErrFuture<F, V>
where
    F: Future<Output = Result<V>>,
{
    type Output = Result<V>;
    fn poll(self: Pin<&mut Self>, waker: &Waker) -> Poll<Self::Output> {
        let this = unsafe { Pin::get_unchecked_mut(self) };

        match &mut this.inner {
            FutureOrErr::Err(e) => Poll::Ready(Err(e.take().unwrap())),
            FutureOrErr::Future(f) => unsafe { Pin::new_unchecked(f) }.poll(waker),
        }
    }
}
