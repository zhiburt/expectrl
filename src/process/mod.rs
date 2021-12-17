#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

use std::io::{Read, Result, Write};

pub trait Process {
    type Stream;

    fn stream(&mut self) -> Result<Self::Stream>;
    fn get_eof_char(&mut self) -> Result<u8>;
    fn get_intr_char(&mut self) -> Result<u8>;
}

pub trait Stream: Write + Read + NonBlocking {}

pub trait NonBlocking {
    fn set_non_blocking(&mut self) -> Result<()>;
    fn set_blocking(&mut self) -> Result<()>;
}
