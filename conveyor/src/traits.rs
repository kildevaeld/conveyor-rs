use super::error::Result;
use super::futures_utils::Promise;
use futures::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context,Poll};

pub trait Station {
    type Input;
    type Output;
    type Future: Future<Output = Result<Self::Output>> + Send;
    fn execute(&self, input: Self::Input) -> Self::Future;
}

pub trait Chain: Sized + Station {
    fn pipe<F: Station<Input = Self::Output> + Send + Sync>(self, f: F) -> Conveyor<Self, F>;
}

impl<T> Chain for T
where
    T: Station + Sync + Send,
{
    fn pipe<F: Station<Input = Self::Output> + Send + Sync>(self, f: F) -> Conveyor<Self, F> {
        Conveyor::new(self, f)
    }
}

#[derive(Clone)]
pub struct Conveyor<F, N>
where
    F: Station,
    N: Station,
{
    f: Arc<F>,
    n: Arc<N>,
}

impl<F, N> Conveyor<F, N>
where
    F: Station + Send + Sync,
    N: Station<Input = F::Output> + Send + Sync,
{
    pub fn new(f: F, n: N) -> Conveyor<F, N> {
        Conveyor {
            f: Arc::new(f),
            n: Arc::new(n),
        }
    }
}

impl<F, N> Station for Conveyor<F, N>
where
    F: Station + Send + Sync,
    N: Station<Input = F::Output> + Send + Sync,
{
    type Input = F::Input;
    type Output = N::Output;
    type Future = ConveyorFuture<F::Future, N, F::Output>;

    fn execute(&self, input: Self::Input) -> Self::Future {
        let fut = self.f.execute(input);
        ConveyorFuture {
            n: self.n.clone(),
            p: Promise::First(fut),
        }
    }
}

pub struct ConveyorFuture<F, N, O>
where
    F: Future<Output = Result<O>>,
    N: Station<Input = O>,
{
    n: Arc<N>,
    p: Promise<F, N::Future>,
}

impl<F, N, O> Future for ConveyorFuture<F, N, O>
where
    F: Future<Output = Result<O>>,
    N: Station<Input = O>,
{
    type Output = Result<N::Output>;
    fn poll(self: Pin<&mut Self>, lw: &mut Context) -> Poll<Self::Output> {
        let mut this = unsafe { Pin::get_unchecked_mut(self) };

        loop {
            let out = match &mut this.p {
                Promise::First(fut) => match unsafe { Pin::new_unchecked(fut) }.poll(lw) {
                    Poll::Pending => Some(Poll::Pending),
                    Poll::Ready(ret) => {
                        if ret.is_err() {
                            Some(Poll::Ready(Err(ret.err().unwrap())))
                        } else {
                            this.p = Promise::Second(this.n.execute(ret.unwrap()));
                            None
                        }
                    }
                },
                Promise::Second(fut) => match unsafe { Pin::new_unchecked(fut) }.poll(lw) {
                    Poll::Pending => Some(Poll::Pending),
                    Poll::Ready(ret) => Some(Poll::Ready(ret)),
                },
            };

            if out.is_some() {
                return out.unwrap();
            }
        }
    }
}

pub struct StationFn<F, I, O> {
    inner: F,
    _i: std::marker::PhantomData<I>,
    _o: std::marker::PhantomData<O>,
}

impl<F, I, O, U> Station for StationFn<F, I, O>
where
    F: (Fn(I) -> U) + Send + Sync + std::marker::Unpin,
    U: Future<Output = Result<O>> + Send + 'static,
{
    type Future = U;
    type Input = I;
    type Output = O;

    fn execute(&self, input: Self::Input) -> Self::Future {
        (self.inner)(input)
    }
}

pub fn station_fn<F, I, O, U>(f: F) -> StationFn<F, I, O>
where
    F: (Fn(I) -> U) + Send + Sync + std::marker::Unpin,
    U: Future<Output = Result<O>> + Send + 'static,
{
    StationFn {
        inner: f,
        _i: std::marker::PhantomData,
        _o: std::marker::PhantomData,
    }
}

pub struct StationFnCtx<F, I, O, Ctx> {
    inner: F,
    ctx: Ctx,
    _i: std::marker::PhantomData<I>,
    _o: std::marker::PhantomData<O>,
}

impl<F, I, O, Ctx, U> Station for StationFnCtx<F, I, O, Ctx>
where
    F: (Fn(I, &Ctx) -> U) + Send + Sync + std::marker::Unpin,
    U: Future<Output = Result<O>> + Send + 'static,
{
    type Future = U;
    type Input = I;
    type Output = O;

    fn execute(&self, input: Self::Input) -> Self::Future {
        (self.inner)(input, &self.ctx)
    }
}

pub fn station_fn_ctx<F, I, O, Ctx: 'static, U>(f: F, ctx: Ctx) -> StationFnCtx<F, I, O, Ctx>
where
    F: (Fn(I, &Ctx) -> U) + Send + Sync + std::marker::Unpin,
    U: Future<Output = Result<O>> + Send + 'static,
{
    StationFnCtx {
        inner: f,
        ctx: ctx,
        _i: std::marker::PhantomData,
        _o: std::marker::PhantomData,
    }
}

pub fn into_box<S: Station + 'static + Send + Sync>(
    i: S,
) -> Box<
    dyn Station<
            Input = S::Input,
            Output = S::Output,
            Future = Pin<Box<dyn Future<Output = Result<S::Output>> + Send>>,
        > + Send
        + Sync,
> {
    Box::new(Boxed { s: i })
}

pub struct Boxed<S: Send + Sync + ?Sized> {
    s: S,
}

impl<S> Station for Boxed<S>
where
    S: Station + Send + Sync + ?Sized,
    <S as Station>::Future: 'static,
{
    type Input = S::Input;
    type Output = S::Output;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Output>> + 'static + Send>>;
    fn execute(&self, input: Self::Input) -> Self::Future {
        Box::pin(self.s.execute(input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn it_works() {
        let s = station_fn(async move |test: String| Ok(test + ", rapper"));
        let s = s.pipe(station_fn(async move |test: String| Ok(test + "!")));
        let p = s.execute("Hello".to_string());
        let ret = block_on(p).unwrap();
        assert_eq!(ret, "Hello, rapper!");
    }

    #[test]
    fn it_works2() {
        let s = station_fn(async move |_test: String| Ok(2))
            .pipe(station_fn(async move |test: i32| Ok(test + 2)));
        let p = s.execute("Hello".to_string());
        let ret = block_on(p).unwrap();
        assert_eq!(ret, 4);
    }

    #[test]
    fn it_works_ctx() {
        let s = station_fn_ctx(async move |_test: String, ctx: &i32| Ok(2), 10)
            .pipe(station_fn(async move |test: i32| Ok(test + 2)));
        let p = s.execute("Hello".to_string());
        let ret = block_on(p).unwrap();
        assert_eq!(ret, 4);
    }

    #[test]
    fn boxed() {
        let s = station_fn(async move |test: &str| Ok(test));
        let b = into_box(s);

        let ret = futures::executor::block_on(b.execute("Hello")).unwrap();
        assert_eq!(ret, "Hello");
    }
}
