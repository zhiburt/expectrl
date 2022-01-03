#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

#[cfg(feature = "async")]
pub mod async_stream;

use std::io::{Read, Result, Write};

pub trait Process {
    type Stream;

    fn stream(&mut self) -> Result<Self::Stream>;
}

#[cfg(not(feature = "async"))]
pub trait Stream: Write + Read + NonBlocking {}

#[cfg(not(feature = "async"))]
pub trait NonBlocking {
    fn set_non_blocking(&mut self) -> Result<()>;
    fn set_blocking(&mut self) -> Result<()>;
}

impl<T> Stream for T where T: Write + Read + NonBlocking {}
