#![feature(async_await, async_closure)]

#[macro_use]
extern crate serde_derive;

#[macro_use]
pub mod macros;

#[cfg(feature = "fs")]
pub mod fs;
#[cfg(feature = "http")]
pub mod http;
pub mod package;
pub mod traits;
pub mod utils;

pub mod producers {

    #[cfg(feature = "http")]
    use super::http::{HttpOptions, HttpProducer};

    #[cfg(feature = "http")]
    pub fn http(options: Vec<HttpOptions>) -> HttpProducer {
        HttpProducer::new(options)
    }

    #[cfg(feature = "fs")]
    pub mod fs {
        use super::super::fs::{FileProducer, FS};
        use vfs::{ReadPath, VFS};

        pub fn glob<V: 'static, S: AsRef<str>>(fs: V, glob: S) -> FileProducer<FS<V>, V::Path>
        where
            V: VFS,
            <V as VFS>::Path: ReadPath,
        {
            let fs = FS::glob(fs, glob);
            FileProducer::new(fs, 2, true)
        }

        pub fn path<V: 'static, S: AsRef<str>>(fs: V, path: S) -> FileProducer<FS<V>, V::Path>
        where
            V: VFS,
            <V as VFS>::Path: ReadPath,
        {
            let fs = FS::path(fs, path);
            FileProducer::new(fs, 2, true)
        }
    }
}

pub mod prelude {
    pub use super::package::Package;
    pub use super::producers as pro;
    //pub use super::work::*;

}
