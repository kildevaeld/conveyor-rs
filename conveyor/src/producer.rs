use super::error::Result;
use super::Station;
use futures::future::{ready, Ready};
use futures::prelude::*;

pub trait Producer {
    type Item: Send;
    type Future: Future<Output = Option<Result<Self::Item>>> + Send;
    fn next(&mut self) -> Self::Future;
}

impl<T: Iterator> Producer for T
where
    <T as Iterator>::Item: Send,
{
    type Item = T::Item;
    type Future = Ready<Option<Result<Self::Item>>>;
    fn next(&mut self) -> Self::Future {
        match <Iterator<Item = T::Item>>::next(self) {
            None => ready(None),
            Some(s) => ready(Some(Ok(s))),
        }
    }
}

pub struct Consumer<P, C> {
    stream: P,
    chain: C,
}

impl<P, C> Consumer<P, C>
where
    P: Producer + Send + 'static,
    C: Station<Input = P::Item> + Send + Sync + 'static,
    <C as Station>::Output: Send + 'static,
{
    pub fn new(producer: P, chain: C) -> Consumer<P, C> {
        Consumer {
            stream: producer,
            chain: chain,
        }
    }

    pub async fn run(&mut self) -> Vec<Result<C::Output>> {
        let mut out = Vec::new();

        loop {
            let next = match await!(self.stream.next()) {
                None => break,
                Some(s) => match s {
                    Ok(m) => await!(self.chain.execute(m)),
                    Err(e) => Err(e),
                },
            };
            out.push(next);
        }
        out
    }
}
