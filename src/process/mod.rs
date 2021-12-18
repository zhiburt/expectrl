#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

#[cfg(feature = "async")]
pub mod async_stream;

use std::io::Result;

pub trait Process {
    type Stream;

    fn stream(&mut self) -> Result<Self::Stream>;
    fn get_eof_char(&mut self) -> Result<u8>;
    fn get_intr_char(&mut self) -> Result<u8>;
}

#[cfg(not(feature = "async"))]
pub trait Stream: std::io::Write + std::io::Read + NonBlocking {}

#[cfg(not(feature = "async"))]
pub trait NonBlocking {
    fn set_non_blocking(&mut self) -> Result<()>;
    fn set_blocking(&mut self) -> Result<()>;
}

#[cfg(feature = "async")]
pub trait Stream: futures_lite::AsyncWrite + futures_lite::AsyncRead + Unpin {}
