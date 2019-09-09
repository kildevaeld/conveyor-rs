use futures::prelude::*;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Poll, Context};

pub struct ConcurrentStream<S>
where
    S: Stream,
    <S as Stream>::Item: Future,
{
    s: S,
    p: Option<Vec<S::Item>>,
    d: VecDeque<<<S as Stream>::Item as Future>::Output>,
    max: usize,
}

impl<S> ConcurrentStream<S>
where
    S: Stream,
    <S as Stream>::Item: Future,
{
    pub fn new(stream: S, max: usize) -> ConcurrentStream<S> {
        ConcurrentStream {
            s: stream,
            p: Some(Vec::new()),
            d: VecDeque::new(),
            max,
        }
    }
}

impl<S> Stream for ConcurrentStream<S>
where
    S: Stream,
    <S as Stream>::Item: Future,
{
    type Item = <<S as Stream>::Item as Future>::Output;

    fn poll_next(self: Pin<&mut Self>, waker: &mut Context) -> Poll<Option<Self::Item>> {
        let this = unsafe { Pin::get_unchecked_mut(self) };

        if this.max == 0 || this.p.as_ref().unwrap().len() < this.max {
            let next = match unsafe { Pin::new_unchecked(&mut this.s) }.poll_next(waker) {
                Poll::Pending => {
                    if this.p.as_ref().unwrap().is_empty() {
                        return Poll::Pending;
                    }
                    None
                }
                Poll::Ready(s) => s,
            };

            if let Some(s) = next {
                this.p.as_mut().unwrap().push(s);
            }
        }

        if !this.p.as_ref().unwrap().is_empty() {
            let iter = this.p.take().unwrap().into_iter();

            let ret = iter
                .filter_map(
                    |mut i| match unsafe { Pin::new_unchecked(&mut i) }.poll(waker) {
                        Poll::Pending => Some(i),
                        Poll::Ready(m) => {
                            this.d.push_back(m);
                            None
                        }
                    },
                )
                .collect::<Vec<_>>();

            this.p = Some(ret);
        }
        if this.p.as_ref().unwrap().is_empty() && this.d.is_empty() {
            return Poll::Ready(None);
        }

        if this.d.is_empty() {
            return Poll::Pending;
        }

        Poll::Ready(this.d.pop_front())
    }
}
