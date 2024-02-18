//! Stream module contains a set of IO (write/read) wrappers.

pub mod log;
pub mod stdin;

use crate::{Captures, Error, Needle};
use std::io;

#[cfg(not(feature = "async"))]
use std::io::{BufRead, Read, Write};

#[cfg(feature = "async")]
use futures_lite::{AsyncBufRead, AsyncRead, AsyncWrite};

/// Trait for types that can read and write to child programs.
#[cfg(not(feature = "async"))]
pub trait StreamSink: Write + Read + BufRead {
    /// Send a buffer to the child program.
    fn send<B: AsRef<[u8]>>(&mut self, buf: B) -> io::Result<()>;

    /// Send a line to the child program.
    fn send_line(&mut self, text: &str) -> io::Result<()>;

    /// Expect output from the child program.
    fn expect<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle;
}

/// Trait for types that can read and write to child programs.
#[cfg(feature = "async")]
#[async_trait::async_trait(?Send)]
pub trait StreamSink: AsyncRead + AsyncWrite + AsyncBufRead + Unpin {
    /// Send a buffer to the child program.
    async fn send<B: AsRef<[u8]>>(&mut self, buf: B) -> io::Result<()>;

    /// Send a line to the child program.
    async fn send_line(&mut self, text: &str) -> io::Result<()>;

    /// Expect output from the child program.
    async fn expect<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle;
}
