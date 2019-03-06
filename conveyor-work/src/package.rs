use conveyor::futures::prelude::*;
use conveyor::{ConveyorError, OneOf4Future, Promise4};
use std::io::Read;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Poll, Waker};

pub enum PackageContent {
    Bytes(Vec<u8>),
    Stream(Pin<Box<Stream<Item = Result<Vec<u8>>> + Send>>),
    Reader(Box<Read + Send>),
    Empty,
}

impl PackageContent {
    pub fn into_future(self) -> impl Future<Output = Result<Vec<u8>>> {
        let o = match self {
            PackageContent::Bytes(b) => Promise4::First(future::ready(Ok(b))),
            PackageContent::Stream(s) => Promise4::Second(ConcatStream {
                stream: s,
                values: Vec::new(),
            }),
            PackageContent::Reader(mut r) => {
                let mut buf = Vec::new();
                let fut = match r.read_to_end(&mut buf) {
                    Ok(_) => future::ready(Ok(buf)),
                    Err(e) => future::ready(Err(ConveyorError::new(e))),
                };
                Promise4::Third(fut)
            }
            PackageContent::Empty => Promise4::Fourth(future::ready(Ok(Vec::new()))),
        };
        OneOf4Future::new(o)
    }
}

impl IntoPackageContent for PackageContent {
    fn into_package_content(self) -> PackageContent {
        self
    }
}

struct ConcatStream<Str, V>
where
    Str: Stream<Item = Result<Vec<V>>>,
{
    stream: Str,
    values: Vec<V>,
}

impl<Str, V> Future for ConcatStream<Str, V>
where
    Str: Stream<Item = Result<Vec<V>>>,
{
    type Output = Result<Vec<V>>;

    fn poll(self: Pin<&mut Self>, waker: &Waker) -> Poll<Self::Output> {
        let this = unsafe { Pin::get_unchecked_mut(self) };

        match unsafe { Pin::new_unchecked(&mut this.stream) }.poll_next(waker) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(o) => match o {
                None => {
                    let values = std::mem::replace(&mut this.values, Vec::new());
                    Poll::Ready(Ok(values))
                }
                Some(s) => match s {
                    Ok(s) => {
                        this.values.extend(s);
                        Poll::Pending
                    }
                    Err(e) => Poll::Ready(Err(e)),
                },
            },
        }
    }
}

pub trait IntoPackageContent {
    fn into_package_content(self) -> PackageContent;
}

impl IntoPackageContent for Vec<u8> {
    fn into_package_content(self) -> PackageContent {
        PackageContent::Bytes(self)
    }
}

impl IntoPackageContent for &[u8] {
    fn into_package_content(self) -> PackageContent {
        PackageContent::Bytes(self.to_vec())
    }
}

impl IntoPackageContent for String {
    fn into_package_content(self) -> PackageContent {
        PackageContent::Bytes(self.as_str().as_bytes().to_vec())
    }
}

impl<'a> IntoPackageContent for &'a str {
    fn into_package_content(self) -> PackageContent {
        PackageContent::Bytes(self.as_bytes().to_vec())
    }
}

impl IntoPackageContent
    for Pin<Box<conveyor::futures::stream::Stream<Item = Result<Vec<u8>>> + Send>>
{
    fn into_package_content(self) -> PackageContent {
        PackageContent::Stream(self)
    }
}

//#[derive(Serialize, Deserialize)]
pub struct Package {
    name: String,
    value: Mutex<Option<PackageContent>>,
}

impl std::fmt::Debug for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut s = f.debug_struct("Package");
        s.field("name", &self.name);
        s.finish()
    }
}

impl Package {
    pub fn new<S: AsRef<str>, V: IntoPackageContent>(name: S, value: V) -> Package {
        Package {
            name: name.as_ref().to_string(),
            value: Mutex::new(Some(value.into_package_content())),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name<S: AsRef<str>>(mut self, name: S) -> Package {
        self.name = name.as_ref().to_string();
        self
    }

    // pub fn value(&self) -> &PackageContent {
    //     &self.value
    // }

    pub fn set_value<S: IntoPackageContent>(mut self, value: S) -> Package {
        self.value = Mutex::new(Some(value.into_package_content()));
        self
    }

    pub async fn read_content(&mut self) -> Result<Vec<u8>> {
        // let body = mem::replace(self.value.lock().unwrap().body_mut(), Decoder::empty());
        let content = std::mem::replace(
            self.value.lock().unwrap().as_mut().unwrap(),
            PackageContent::Empty,
        );
        await!(content.into_future())
    }
}

use conveyor::futures::future::{ready, Ready};
use conveyor::Result;
use conveyor::Station;

pub struct ToPackage<T> {
    name: String,
    _i: std::marker::PhantomData<T>,
}

impl<T: IntoPackageContent> Station for ToPackage<T> {
    type Input = T;
    type Output = Package;
    type Future = Ready<Result<Self::Output>>;

    fn execute(&self, input: Self::Input) -> Self::Future {
        ready(Ok(Package::new(&self.name, input)))
    }
}

pub fn to_package<S: AsRef<str>, T: IntoPackageContent>(name: S) -> ToPackage<T> {
    ToPackage {
        name: name.as_ref().to_string(),
        _i: std::marker::PhantomData,
    }
}
