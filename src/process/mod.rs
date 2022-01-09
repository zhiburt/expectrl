#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

use std::io::{Read, Result, Write};

pub trait Process {
    type Stream;

    fn stream(&mut self) -> Result<Self::Stream>;
}

pub trait Stream: Write + Read + NonBlocking {}

pub trait NonBlocking {
    fn set_non_blocking(&mut self) -> Result<()>;
    fn set_blocking(&mut self) -> Result<()>;
}

impl<T> Stream for T where T: Write + Read + NonBlocking {}
