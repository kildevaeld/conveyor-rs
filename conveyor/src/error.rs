use std::error::Error;
use std::fmt;
use std::result;

pub type Result<T> = result::Result<T, ConveyorError>;

pub struct ConveyorError {
    inner: Box<dyn Error + 'static>,
}

impl ConveyorError {
    pub fn new<E: Error + 'static>(error: E) -> ConveyorError {
        ConveyorError {
            inner: Box::new(error),
        }
    }
}

impl fmt::Debug for ConveyorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ConveyorError: {:?}", self.inner)
    }
}

impl fmt::Display for ConveyorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "")
    }
}

impl Error for ConveyorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.inner.as_ref())
    }
}
