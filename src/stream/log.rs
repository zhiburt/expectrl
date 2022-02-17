//! A wrapper of [Session] to log a read/write operations
use std::{
    io::{self, Read, Result, Write},
    ops::{Deref, DerefMut},
};

use crate::session::sync_stream::NonBlocking;

#[cfg(feature = "async")]
use futures_lite::{AsyncRead, AsyncWrite};
#[cfg(feature = "async")]
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pub struct LoggedStream<S, W> {
    stream: S,
    logger: W,
}

impl<S, W> LoggedStream<S, W> {
    pub fn new(stream: S, logger: W) -> Self {
        Self { stream, logger }
    }
}

impl<S, W: Write> LoggedStream<S, W> {
    fn log_write(&mut self, buf: &[u8]) {
        log(&mut self.logger, "write", buf);
    }

    fn log_read(&mut self, buf: &[u8]) {
        log(&mut self.logger, "read", buf);
    }
}

impl<S: Write, W: Write> Write for LoggedStream<S, W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let n = self.stream.write(buf)?;
        self.log_write(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> Result<()> {
        self.stream.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> Result<usize> {
        let n = self.stream.write_vectored(bufs)?;

        let mut rest = n;
        let mut bytes = Vec::new();
        for buf in bufs {
            let written = std::cmp::min(buf.len(), rest);
            rest -= written;

            bytes.extend(&buf.as_ref()[..written]);

            if rest == 0 {
                break;
            }
        }

        self.log_write(&bytes);

        Ok(n)
    }
}

impl<S: Read, W: Write> Read for LoggedStream<S, W> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = self.stream.read(buf)?;
        self.log_read(&buf[..n]);
        Ok(n)
    }
}

impl<S: NonBlocking, W> NonBlocking for LoggedStream<S, W> {
    fn set_non_blocking(&mut self) -> Result<()> {
        self.stream.set_non_blocking()
    }

    fn set_blocking(&mut self) -> Result<()> {
        self.stream.set_blocking()
    }
}

impl<S, W> Deref for LoggedStream<S, W> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<S, W> DerefMut for LoggedStream<S, W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

#[cfg(feature = "async")]
impl<S: AsyncWrite + Unpin, W: Write + Unpin> AsyncWrite for LoggedStream<S, W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.log_write(buf);
        Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_close(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write_vectored(cx, bufs)
    }
}

#[cfg(feature = "async")]
impl<S: AsyncRead + Unpin, W: Write + Unpin> AsyncRead for LoggedStream<S, W> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.stream).poll_read(cx, buf);
        if let Poll::Ready(Ok(n)) = &result {
            self.log_read(&buf[..*n]);
        }

        result
    }
}

fn log(mut writer: impl Write, target: &str, data: &[u8]) {
    let _ = match std::str::from_utf8(data) {
        Ok(data) => writeln!(writer, "{}: {:?}", target, data),
        Err(..) => writeln!(writer, "{}:(bytes): {:?}", target, data),
    };
}
