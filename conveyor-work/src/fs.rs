use super::package::{Package, PackageContent};
use conveyor::futures_old::{
    sync::mpsc::{channel, Receiver as FutureReceiver, Sender as FutureSender},
    Async, Future as OldFuture, Sink, Stream,
};
use conveyor::*;
use std::io::Read;
use std::pin::Pin;
use std::task::{Poll, Waker};
use std::thread;
use vfs::boxed::*;
use vfs::prelude::*;

pub enum FSRequest {
    Path(String),
    Glob(String),
}

pub struct FS<V: VFS>
where
    <V as VFS>::Path: ReadPath,
{
    rx: FutureReceiver<Result<V::Path>>,
}

impl<V: VFS> FS<V>
where
    V: VFS + 'static,
    <V as VFS>::Path: ReadPath,
{
    pub fn glob<S: AsRef<str>>(fs: V, glob: S) -> FS<V> {
        let (sx, rx) = channel::<Result<V::Path>>(0);
        let glob = glob.as_ref().to_string();
        tokio::spawn_async(
            async move {
                for i in fs.path("").glob_walk(glob) {
                    tokio::await!(sx.clone().send(Ok(i))).expect("could not send");
                }
            },
        );

        FS { rx }
    }

    pub fn path<S: AsRef<str>>(fs: V, path: S) -> FS<V> {
        let (sx, rx) = channel::<Result<V::Path>>(0);
        let path = path.as_ref().to_string();
        tokio::spawn_async(
            async move {
                for i in fs.path(&path).walk_dir() {
                    tokio::await!(sx.clone().send(Ok(i))).expect("could not send");
                }
            },
        );

        FS { rx }
    }
}

impl<V: VFS> futures::Stream for FS<V>
where
    <V as VFS>::Path: ReadPath,
{
    type Item = Result<V::Path>;

    fn poll_next(self: Pin<&mut Self>, _waker: &Waker) -> Poll<Option<Self::Item>> {
        let this = unsafe { Pin::get_unchecked_mut(self) };

        match this.rx.poll() {
            Ok(m) => match m {
                Async::NotReady => Poll::Pending,
                Async::Ready(ret) => Poll::Ready(ret),
            },
            Err(_) => panic!("should not happen"),
        }
    }
}

pub struct FileProducer<Str: futures::Stream<Item = Result<V>>, V: ReadPath> {
    stream: Str,
    station: WorkStation<V, Package>,
}

impl<Str: futures::Stream<Item = Result<V>>, V: ReadPath + 'static> FileProducer<Str, V> {
    pub fn new(stream: Str, nwork: usize, read: bool) -> FileProducer<Str, V> {
        FileProducer {
            stream,
            station: WorkStation::new(
                nwork,
                move |path: V, _ctx: &mut ()| match path.open() {
                    Err(e) => return Err(ConveyorError::new(e)),
                    Ok(mut file) => {
                        let content = if read {
                            let mut buf = Vec::new();
                            match file.read_to_end(&mut buf) {
                                Err(e) => return Err(ConveyorError::new(e)),
                                Ok(_) => PackageContent::Bytes(buf),
                            }
                        } else {
                            PackageContent::Reader(Box::new(file))
                        };
                        Ok(Package::new(path.to_string(), content))
                    }
                },
                || (),
            ),
        }
    }
}

impl<Str: futures::Stream<Item = Result<V>>, V: ReadPath + 'static> futures::Stream
    for FileProducer<Str, V>
{
    type Item = OneOfFuture<
        OneOfFuture<
            WorkStationFuture<Package>,
            futures::future::Ready<Result<Package>>,
            Result<Package>,
        >,
        futures::future::Ready<Result<Package>>,
        Result<Package>,
    >;
    fn poll_next(self: Pin<&mut Self>, waker: &Waker) -> Poll<Option<Self::Item>> {
        let this = unsafe { Pin::get_unchecked_mut(self) };

        match unsafe { Pin::new_unchecked(&mut this.stream) }.poll_next(waker) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(s) => match s {
                None => Poll::Ready(None),
                Some(m) => match m {
                    Err(e) => Poll::Ready(Some(OneOfFuture::new(Promise::Second(
                        futures::future::ready(Err(e)),
                    )))),
                    Ok(ret) => Poll::Ready(Some(OneOfFuture::new(Promise::First(
                        this.station.execute(ret),
                    )))),
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {

    use super::super::traits::ChainStreamer;
    use super::*;
    use conveyor::futures::prelude::*;
    use conveyor::station_fn;
    use conveyor::{ConcurrentStream, WorkStation};
    use tokio;
    use vfs::physical;

    #[test]
    fn test_vfs() {
        tokio::run_async(
            async {
                let fs = FS::glob(physical::PhysicalFS::new("../").unwrap(), "**/*.rs");

                let fs = FileProducer::new(fs, 1, true);

                let fs = ConcurrentStream::new(fs, 4);

                // let fs = fs.then(async move |p: Result<Package>| {
                //     let mut p = match p {
                //         Ok(p) => p,
                //         Err(e) => return Err(e),
                //     };
                //     let content = await!(p.read_content())?;
                //     Ok(p.set_value(content))
                // });

                await!(fs
                    .then(async move |m| {
                        let (content, name) = match m {
                            Ok(mut m) => (await!(m.read_content()), m.name().to_string()),
                            Err(e) => return (),
                        };
                        //let content = await!(m..read_content());
                        println!("{}", name);
                    })
                    .collect::<Vec<_>>());
            },
        );
    }
}
