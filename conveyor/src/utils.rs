use super::{ConveyorError, Result, Station};
use futures::future::{ready, Ready};
use std::thread;

#[derive(Clone, Debug)]
pub struct ToString;

impl Station for ToString {
    type Input = Vec<u8>;
    type Output = String;
    type Future = Ready<Result<String>>;
    fn execute(&self, input: Self::Input) -> Self::Future {
        ready(String::from_utf8(input).map_err(|e| e.into()))
    }
}

pub fn to_string() -> ToString {
    ToString
}
