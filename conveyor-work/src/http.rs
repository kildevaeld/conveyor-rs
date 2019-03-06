use super::package::{to_package, Package, ToPackage};
use super::utils::{BoxWrap, BoxedStation};
use conveyor::futures::prelude::*;
use conveyor::{Chain, ConveyorError, ConveyorFuture, Result, Station};
use conveyor_http::{HeaderMap, HttpFuture, HttpResponse, HttpResponseReader};
use hyper_serde;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Poll, Waker};
use url::Url;
use url_serde;

macro_rules! method_impl {
    ($name: ident, $method: ident) => {
        pub fn $name<S: AsRef<str>>(url: S) -> Result<HttpOptions> {
            let url = match Url::parse(url.as_ref()) {
                Ok(url) => url,
                Err(e) => return Err(ConveyorError::new(e)),
            };

            Ok(HttpOptions::new(Method::$method, url))
        }
    };
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Method {
    GET,
    POST,
    PUT,
    DELETE,
}

#[derive(Serialize, Deserialize)]
pub struct HttpOptions {
    pub method: Method,
    #[serde(with = "url_serde")]
    pub url: Url,
    #[serde(
        deserialize_with = "hyper_serde::deserialize",
        serialize_with = "hyper_serde::serialize"
    )]
    pub headers: HeaderMap,
    #[serde(skip)]
    pub station: Option<BoxedStation>,
}

impl HttpOptions {
    pub fn new(method: Method, url: Url) -> HttpOptions {
        HttpOptions {
            method,
            url,
            headers: HeaderMap::new(),
            station: None,
        }
    }

    method_impl!(get, GET);
    method_impl!(post, POST);
    method_impl!(put, PUT);
    method_impl!(delete, DELETE);

    pub fn to_request(self) -> conveyor_http::Request {
        let method = match self.method {
            Method::GET => conveyor_http::Method::GET,
            Method::POST => conveyor_http::Method::POST,
            Method::PUT => conveyor_http::Method::PUT,
            Method::DELETE => conveyor_http::Method::DELETE,
        };
        let mut ret = conveyor_http::Request::new(method, self.url);
        *ret.headers_mut() = self.headers;
        ret
    }

    pub fn station<S: Station<Input = Package, Output = Package> + Send + Sync + 'static>(
        mut self,
        station: S,
    ) -> Self {
        self.station = Some(conveyor::into_box(station));
        self
    }
}

pub struct HttpProducer {
    client: conveyor_http::Http,
    request: VecDeque<HttpOptions>,
}

impl HttpProducer {
    pub fn new(requests: Vec<HttpOptions>) -> HttpProducer {
        HttpProducer {
            client: conveyor_http::Http::new(),
            request: VecDeque::from(requests),
        }
    }
}

impl Stream for HttpProducer {
    type Item = ConveyorFuture<
        ConveyorFuture<
            ConveyorFuture<HttpFuture, HttpResponseReader, HttpResponse>,
            ToPackage<Vec<u8>>,
            Vec<u8>,
        >,
        BoxWrap,
        Package,
    >;
    // type Item = ConveyorFuture<
    //     ConveyorFuture<
    //         ConveyorFuture<HttpFuture, HttpResponseStream, HttpResponse>,
    //         ToPackage<
    //             std::sync::Mutex<
    //                 Pin<Box<conveyor::futures::stream::Stream<Item = Result<Vec<u8>>> + Send>>,
    //             >,
    //         >,
    //         std::sync::Mutex<
    //             Pin<Box<conveyor::futures::stream::Stream<Item = Result<Vec<u8>>> + Send>>,
    //         >,
    //     >,
    //     BoxWrap,
    //     Package,
    // >;

    fn poll_next(self: Pin<&mut Self>, _waker: &Waker) -> Poll<Option<Self::Item>> {
        let this = unsafe { Pin::get_unchecked_mut(self) };

        if this.request.is_empty() {
            return Poll::Ready(None);
        }

        let mut next = this.request.pop_front().unwrap();

        let chain = this
            .client
            .clone()
            .pipe(HttpResponseReader)
            //.pipe(HttpResponseStream)
            .pipe(to_package(next.url.as_str()));

        let chain = if next.station.is_some() {
            let m = next.station.take().unwrap();
            chain.pipe(boxwrap!(m))
        } else {
            chain.pipe(boxwrap!(conveyor::into_box(conveyor::station_fn(
                async move |p: Package| Ok(p),
            ))))
        };

        let ret = chain.execute(next.to_request());
        Poll::Ready(Some(ret))
    }
}

#[cfg(test)]
mod tests {

    use super::super::traits::ChainStreamer;
    use super::*;
    use conveyor::station_fn;
    use conveyor::{ConcurrentStream, WorkStation};
    use tokio;

    #[test]
    fn it_works() {
        tokio::run_async(
            async {
                let producer = HttpProducer::new(vec![
                    HttpOptions::get("https://skuffesalg.nu/")
                        .unwrap()
                        .station(station_fn(async move |p: Package| {
                            println!("skuffe salg nu");
                            Ok(p)
                        })),
                    HttpOptions::get("https://www.telmore.dk/").unwrap(),
                    HttpOptions::get("https://www.valdemarsro.dk/blinis-med-stenbiderrogn/")
                        .unwrap(),
                    HttpOptions::get("https://distrowatch.org").unwrap(),
                    HttpOptions::get("https://bolighed.dk").unwrap(),
                ]);

                let stream = producer
                    .pipe(station_fn(async move |mut m: Package| {
                        println!("done {}", m.name());
                        let name = m.name().to_string();
                        let value = await!(m.read_content())?;
                        println!("done2 {}", m.name());
                        Ok(Package::new(name, value))
                    }))
                    .pipe(WorkStation::new(
                        4,
                        |p: Package, ctx: &mut String| {
                            println!("thread {:?} {}", std::thread::current().id(), p.name());
                            Ok(p)
                        },
                        || String::from("Hello"),
                    ));

                let stream = ConcurrentStream::new(stream, 4);

                let ret = await!(stream.collect::<Vec<_>>());

                for r in ret {
                    println!("ret {}", r.unwrap().name());
                }
            },
        );
    }
}
