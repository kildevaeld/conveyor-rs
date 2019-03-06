use super::concurrent_stream::ConcurrentStream;
use super::{into_box, Result, Station};
use futures::prelude::*;
use std::pin::Pin;

pub enum WorkOutput<V> {
    Result(V),
    Work(Work<V>),
}

type WorkBox<V> = Box<
    Station<
            Input = V,
            Output = Vec<WorkOutput<V>>,
            Future = Pin<Box<Future<Output = Result<Vec<WorkOutput<V>>>> + Send>>,
        > + Send
        + Sync,
>;

pub struct Work<V> {
    data: V,
    work: WorkBox<V>,
}

impl<V> Work<V> {
    pub fn new<
        W: Station<Input = V, Output = Vec<WorkOutput<V>>, Future = F> + 'static + Send + Sync,
        F: Future<Output = Result<Vec<WorkOutput<V>>>> + Send,
    >(
        data: V,
        work: W,
    ) -> Work<V> {
        Work {
            data,
            work: into_box(work),
        }
    }

    pub fn chain<
        W: Station<Input = V, Output = Vec<WorkOutput<V>>, Future = F> + 'static + Send + Sync,
        F: Future<Output = Result<Vec<WorkOutput<V>>>> + Send,
    >(
        self,
        _next: W,
    ) -> Work<V> {
        Work {
            data: self.data,
            work: self.work,
        }
    }
}

pub struct Worker;

impl Worker {
    pub fn new() -> Worker {
        Worker
    }

    fn _run<'a, V: 'static>(
        &'a self,
        input: Vec<Work<V>>,
    ) -> impl Future<Output = Vec<Result<Vec<WorkOutput<V>>>>> {
        let stream = futures::stream::iter(input);
        ConcurrentStream::new(stream.map(|work| work.work.execute(work.data)), 4).collect()
    }

    pub async fn run<V: 'static>(&self, input: Vec<Work<V>>) -> Vec<Result<V>> {
        let mut ret = input;
        let mut output = Vec::new();
        loop {
            ret = await!(self._run(ret))
                .into_iter()
                .filter_map(|m| match m {
                    Ok(s) => Some(
                        s.into_iter()
                            .filter_map(|m| match m {
                                WorkOutput::Result(r) => {
                                    output.push(Ok(r));
                                    None
                                }
                                WorkOutput::Work(w) => Some(w),
                            })
                            .collect::<Vec<_>>(),
                    ),
                    Err(e) => {
                        output.push(Err(e));
                        None
                    }
                })
                .flatten()
                .collect();

            if ret.is_empty() {
                break;
            }
        }
        output
    }
}

#[cfg(test)]
mod tests {

    use super::super::*;
    use super::*;
    use futures;

    #[test]
    fn it_works() {
        let work = Work::new(
            String::from("Value, baby!"),
            station_fn(async move |val| Ok(vec![WorkOutput::Result(val)])),
        );

        let worker = Worker::new();

        let ret = futures::executor::block_on(worker.run(vec![work]));

        assert_eq!(ret.len(), 1);
        assert!(ret[0].is_ok());
        //assert_eq!(&ret[0].unwrap(), String::from("Value, baby!"));
    }

    #[test]
    fn more_work() {
        let work = Work::new(
            String::from("Value, baby!"),
            station_fn(async move |val: String| {
                Ok(vec![
                    WorkOutput::Result(val),
                    WorkOutput::Work(Work::new(
                        String::from("Value, baby 2!"),
                        station_fn(async move |val| Ok(vec![WorkOutput::Result(val)])),
                    )),
                ])
            }),
        );

        let worker = Worker::new();

        let ret = futures::executor::block_on(worker.run(vec![work]));

        assert_eq!(ret.len(), 2);
        //assert_eq!(&ret[0].unwrap(), String::from("Value, baby!"));
    }

    // #[test]
    // fn tokio_work() {
    //     tokio::run_async(
    //         async {
    //             let http = Http::new();

    //             let worker = Worker::new();

    //             let ret = await!(worker.run(vec![Work::new(
    //                 "https://distrowatch.com".to_string(),
    //                 station_fn(async move |val: String| {
    //                     let url = Url::parse(val.as_str()).unwrap();
    //                     let req = Request::new(Method::GET, url);
    //                     let result = await!(Http::new()
    //                         .chain(HttpResponseReader)
    //                         .chain(utils::to_string())
    //                         .execute(req))?;
    //                     Ok(vec![WorkOutput::Result(result)])
    //                 }),
    //             )]));

    //             for r in ret {
    //                 println!("{:?}", r);
    //             }
    //         },
    //     );
    // }

}
