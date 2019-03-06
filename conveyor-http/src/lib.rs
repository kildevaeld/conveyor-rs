#![feature(async_await, await_macro, futures_api)]

use conveyor::futures::Stream as StdStream;
use conveyor::futures_old::{Async, Future, Stream};
use conveyor::{ConveyorError, Result, Station};
pub use reqwest::header::HeaderMap;
use reqwest::r#async::{Client, Decoder};
use reqwest::{Error, IntoUrl, StatusCode};
use std::future::Future as StdFuture;
use std::mem;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker as LocalWaker};

pub use reqwest::{
    r#async::{Chunk, Request, Response},
    Method, Url,
};

#[derive(Clone, Debug)]
pub struct Http {
    client: Arc<Client>,
}

impl Http {
    pub fn new() -> Http {
        Http {
            client: Arc::new(Client::new()),
        }
    }

    pub fn get<U: IntoUrl>(&self, url: U) -> HttpFuture {
        self.execute(Request::new(Method::GET, url.into_url().unwrap()))
    }
}

impl Station for Http {
    type Input = Request;
    type Output = HttpResponse;
    type Future = HttpFuture;

    fn execute(&self, input: Self::Input) -> Self::Future {
        HttpFuture {
            inner: Box::new(self.client.execute(input)),
        }
    }
}

pub struct HttpFuture {
    inner: Box<Future<Item = Response, Error = Error> + Send>,
}

impl StdFuture for HttpFuture {
    type Output = Result<HttpResponse>;
    fn poll(self: Pin<&mut Self>, _lw: &LocalWaker) -> Poll<Self::Output> {
        let this = unsafe { Pin::get_unchecked_mut(self) };

        match this.inner.poll() {
            Ok(Async::NotReady) => Poll::Pending,
            Ok(Async::Ready(r)) => Poll::Ready(Ok(HttpResponse {
                inner: Mutex::new(r),
            })),
            Err(e) => Poll::Ready(Err(ConveyorError::new(e))),
        }
    }
}

#[derive(Debug)]
pub struct HttpResponse {
    inner: Mutex<Response>,
}

impl HttpResponse {
    pub fn status(&self) -> StatusCode {
        self.inner.lock().unwrap().status()
    }

    pub fn headers(&self) -> HeaderMap {
        self.inner.lock().unwrap().headers().clone()
    }

    pub fn read_body(&mut self) -> HttpBodyFuture<Vec<u8>> {
        let body = mem::replace(self.inner.lock().unwrap().body_mut(), Decoder::empty());
        let fut = body.concat2().map(|m| m.to_vec());
        HttpBodyFuture(Box::new(fut))
    }

    pub fn stream(&mut self) -> HttpBodyStream<Vec<u8>> {
        let body = mem::replace(self.inner.lock().unwrap().body_mut(), Decoder::empty());
        HttpBodyStream(Box::new(body.map(|m| m.to_vec())))
    }
}

pub struct HttpBodyStream<U>(Box<Stream<Item = U, Error = Error> + Send + 'static>);

impl<U> StdStream for HttpBodyStream<U> {
    type Item = Result<U>;
    fn poll_next(self: Pin<&mut Self>, _waker: &LocalWaker) -> Poll<Option<Self::Item>> {
        let this = unsafe { Pin::get_unchecked_mut(self) };
        match this.0.poll() {
            Ok(Async::NotReady) => Poll::Pending,
            Ok(Async::Ready(r)) => match r {
                Some(m) => Poll::Ready(Some(Ok(m))),
                None => Poll::Ready(None),
            },
            Err(e) => Poll::Ready(Some(Err(ConveyorError::new(e)))),
        }
    }
}

pub struct HttpBodyFuture<U>(Box<Future<Item = U, Error = Error> + Send>);

impl<U> StdFuture for HttpBodyFuture<U> {
    type Output = Result<U>;
    fn poll(self: Pin<&mut Self>, _lw: &LocalWaker) -> Poll<Self::Output> {
        let this = unsafe { Pin::get_unchecked_mut(self) };
        match this.0.poll() {
            Ok(Async::NotReady) => Poll::Pending,
            Ok(Async::Ready(r)) => Poll::Ready(Ok(r)),
            Err(e) => Poll::Ready(Err(ConveyorError::new(e))),
        }
    }
}

#[derive(Clone, Debug)]
pub struct HttpResponseReader;

impl Station for HttpResponseReader {
    type Input = HttpResponse;
    type Output = Vec<u8>;
    type Future = HttpBodyFuture<Vec<u8>>;
    fn execute(&self, mut input: Self::Input) -> Self::Future {
        input.read_body()
    }
}

#[derive(Clone, Debug)]
pub struct HttpResponseStream;

impl Station for HttpResponseStream {
    type Input = HttpResponse;
    type Output =
        Mutex<Pin<Box<conveyor::futures::stream::Stream<Item = Result<Vec<u8>>> + Send + 'static>>>;
    type Future = conveyor::futures::future::Ready<Result<Self::Output>>;
    fn execute(&self, mut input: Self::Input) -> Self::Future {
        conveyor::futures::future::ready(Ok(Mutex::new(Box::pin(input.stream()))))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
