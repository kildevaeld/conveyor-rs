#![feature(async_await, await_macro, futures_api)]

use conveyor::{ConveyorError, Result, Station};
use futures::{Async, Future, Stream};
use http::HeaderMap;
use reqwest::r#async::{Body, Chunk, Client, Decoder, Request, Response};
use reqwest::{Error, IntoUrl, Method, Result as HttpResult, StatusCode};
use std::future::Future as StdFuture;
use std::mem;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Poll, Waker as LocalWaker};

use std::thread;

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

    pub fn get<U: IntoUrl>(
        &self,
        url: U,
    ) -> HttpFuture<Box<Future<Item = Response, Error = Error> + Send + 'static>> {
        self.execute(Request::new(Method::GET, url.into_url().unwrap()))
    }
}

impl Station for Http {
    type Input = Request;
    type Output = HttpResponse;
    type Future = HttpFuture<Box<Future<Item = Response, Error = Error> + Send>>;

    fn execute(&self, input: Self::Input) -> Self::Future {
        HttpFuture {
            inner: Box::new(self.client.execute(input)),
        }
    }
}

pub struct HttpFuture<F> {
    inner: F,
}

impl<F> StdFuture for HttpFuture<F>
where
    F: Future<Item = Response, Error = Error> + Send,
{
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

    pub fn read_body(
        &mut self,
    ) -> HttpBodyFuture<Box<Future<Item = Vec<u8>, Error = Error> + Send>, Vec<u8>> {
        let body = mem::replace(self.inner.lock().unwrap().body_mut(), Decoder::empty());
        let fut = body.concat2().map(|m| m.to_vec());
        HttpBodyFuture(Box::new(fut))
    }
}

pub struct HttpBodyFuture<F: Future<Item = U, Error = Error> + Send, U>(F);

impl<F, U> StdFuture for HttpBodyFuture<F, U>
where
    F: Future<Item = U, Error = Error> + Send,
{
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
    type Future = HttpBodyFuture<Box<Future<Item = Vec<u8>, Error = Error> + Send>, Vec<u8>>;
    fn execute(&self, mut input: Self::Input) -> Self::Future {
        input.read_body()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
