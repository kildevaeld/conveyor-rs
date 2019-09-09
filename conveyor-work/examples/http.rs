#![feature(async_await, async_closure)]


use conveyor_work::http::*;
use conveyor_work::prelude::*;
use conveyor_http::*;
   use conveyor_work::traits::ChainStreamer;

    use conveyor::*;
    use conveyor::{ConcurrentStream, WorkStation};
    
use tokio;
use tokio::io::AsyncWriteExt;
    use std::error::Error;
    use std::result::Result;
use conveyor::futures::prelude::*;
   
    #[tokio::main]
    pub async fn main() -> Result<(), Box<dyn Error>>  {
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

                // let stream = producer
                //     .pipe(station_fn(async move |mut m: Package| {
                //         println!("done {}", m.name());
                //         let name = m.name().to_string();
                //         let value = m.read_content().await?;
                //         println!("done2 {}", m.name());
                //         Ok(Package::new(name, value))
                //     }));
                    // .pipe(WorkStation::new(
                    //     4,
                    //     |p: Package, ctx: &mut String| {
                    //         println!("thread {:?} {}", std::thread::current().id(), p.name());
                    //         Ok(p)
                    //     },
                    //     || String::from("Hello"),
                    // ));

                // let stream = ConcurrentStream::new(stream, 4);

                let ret = producer.then(|m| m).collect::<Vec<_>>().await;

                for r in ret {
                    println!("ret {}", r.unwrap().name());
                }

                Ok(())
    }