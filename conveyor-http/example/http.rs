#![feature(async_await, await_macro, futures_api)]

use conveyor::producer::*;
use conveyor::*;
use conveyor_http::*;
use reqwest::r#async::Request;
use reqwest::{Method, Url};
use tokio;

fn main() {
    tokio::run_async(
        async move {
            let h = Http::new();

            // let h = h.chain(station_fn(async move |mut res: HttpResponse| {
            //     let body = await!(res.read_body())?;
            //     Ok(String::from_utf8(body.to_vec()).unwrap())
            // }));

            let h = h.chain(HttpResponseReader).chain(utils::to_string());
            let clone = h.clone();
            let req = Request::new(Method::GET, Url::parse("https://distrowatch.com/").unwrap());

            let urls = vec![
                "https://distrowatch.com/".to_string(),
                "https://google.com".to_string(),
                "https://bolighed.dk".to_string(),
                "https://distrowatch.com/".to_string(),
                "https://google.com".to_string(),
                "https://bolighed.dk".to_string(),
            ]
            .into_iter()
            .map(|m| Url::parse(&m).unwrap())
            .map(|m| Request::new(Method::GET, m));

            let mut consumer = Consumer::new(urls, h);

            let ret = await!(consumer.run())
                .iter()
                .filter_map(|m| match m {
                    Err(_) => None,
                    Ok(m) => Some(m.as_str()),
                })
                .collect::<Vec<_>>()
                .join("\n");

            println!("ret {}", ret);

            // .map(|m| h.execute(m))
            // .collect::<Vec<_>>();

            //let fut = conveyor::futures::join!(urls);

            // let body = await!(h.execute(req)).unwrap();
            // // let mut f = await!(h.get("https://distrowatch.com/")).unwrap();
            // // let body = await!(f.read_body()).unwrap();
            // // let body = String::from_utf8(body.to_vec()).unwrap();
            // println!("body {}", body);
        },
    );
}
