use super::futures_utils::{OneOfFuture, Promise};
use super::{ConveyorError, Result, Station};
use crossbeam;
use futures::prelude::*;
use futures_old::{
    sync::oneshot::{channel, Receiver as FutureReceiver, Sender as FutureSender},
    Async, Future as OldFuture,
};
use std::pin::Pin;
use std::task::{Poll, Waker};
use std::thread;

pub struct WorkStation<V: Send, O: Send> {
    sx: crossbeam::channel::Sender<Option<(V, FutureSender<Result<O>>)>>,
    txs: Option<Vec<thread::JoinHandle<()>>>,
}

impl<V: Send + 'static, O: Send + 'static> WorkStation<V, O> {
    pub fn new<F, C: Send + 'static, Cinit>(nwork: usize, work: F, ctx: Cinit) -> WorkStation<V, O>
    where
        F: (Fn(V, &mut C) -> Result<O>) + Clone + Send + 'static,
        Cinit: (Fn() -> C),
    {
        let mut workers = Vec::new();
        let (sx, rx) = crossbeam::channel::bounded::<Option<(V, FutureSender<Result<O>>)>>(nwork);
        for _i in 0..nwork {
            let w = work.clone();
            let mut c = ctx();
            let rxc = rx.clone();

            workers.push(thread::spawn(move || loop {
                let (ret, sx) = match rxc.recv() {
                    Ok(ret) => match ret {
                        Some(ret) => ret,
                        None => {
                            return;
                        }
                    },
                    Err(_) => {
                        return;
                    }
                };
                sx.send(w(ret, &mut c));
            }));
        }

        WorkStation {
            sx: sx,
            txs: Some(workers),
        }
    }
}

impl<V: Send, O: Send> Drop for WorkStation<V, O> {
    fn drop(&mut self) {
        for _ in 0..self.txs.as_ref().unwrap().len() {
            self.sx.send(None).unwrap();
        }
        for w in self.txs.take().unwrap().into_iter() {
            w.join().expect("could not join handle");
        }
    }
}

impl<V: Send + 'static, O: Send + 'static> Station for WorkStation<V, O> {
    type Input = V;
    type Output = O;
    type Future = OneOfFuture<WorkStationFuture<O>, future::Ready<Result<O>>, Result<O>>;

    fn execute(&self, input: Self::Input) -> Self::Future {
        let (sx, rx) = channel();
        match self.sx.send(Some((input, sx))) {
            Ok(_) => OneOfFuture::new(Promise::First(WorkStationFuture { inner: rx })),
            Err(e) => OneOfFuture::new(Promise::Second(future::ready(Err(ConveyorError::new(e))))),
        }
    }
}

pub struct WorkStationFuture<O: Send + 'static> {
    inner: FutureReceiver<Result<O>>,
}

impl<O: Send + 'static> Future for WorkStationFuture<O> {
    type Output = Result<O>;
    fn poll(self: Pin<&mut Self>, _waker: &Waker) -> Poll<Self::Output> {
        let this = unsafe { Pin::get_unchecked_mut(self) };

        match this.inner.poll() {
            Ok(m) => match m {
                Async::NotReady => Poll::Pending,
                Async::Ready(m) => Poll::Ready(m),
            },
            Err(e) => Poll::Ready(Err(ConveyorError::new(e))),
        }
    }
}
