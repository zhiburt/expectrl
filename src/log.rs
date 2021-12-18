//! A wrapper of [Session] to log a read/write operations
use std::{
    io::{self, Read, Result, Write},
    ops::{Deref, DerefMut},
};

use crate::process::Stream;

#[cfg(feature = "async")]
use std::{
    pin::Pin,
    task::{Context, Poll},
};

#[cfg(feature = "async")]
use futures_lite::{AsyncRead, AsyncWrite};

pub struct LoggedStream<S, L> {
    stream: S,
    logger: L,
}

impl<S, L: Write> LoggedStream<S, L> {
    pub fn new(stream: S, logger: L) -> Self {
        Self { stream, logger }
    }

    fn log_write(&mut self, buf: &[u8]) {
        log(&mut self.logger, "write", buf);
    }

    fn log_read(&mut self, buf: &[u8]) {
        log(&mut self.logger, "read", buf);
    }
}

impl<S: Write, L: Write> Write for LoggedStream<S, L> {
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

impl<S: Read, L: Write> Read for LoggedStream<S, L> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = self.stream.read(buf)?;
        self.log_read(&buf[..n]);
        Ok(n)
    }
}

impl<S, L> Deref for LoggedStream<S, L> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<S, L> DerefMut for LoggedStream<S, L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

#[cfg(not(feature = "async"))]
impl<S: crate::process::NonBlocking, L> crate::process::NonBlocking for LoggedStream<S, L> {
    fn set_non_blocking(&mut self) -> Result<()> {
        self.stream.set_non_blocking()
    }

    fn set_blocking(&mut self) -> Result<()> {
        self.stream.set_blocking()
    }
}

#[cfg(feature = "async")]
impl<S: AsyncWrite + Unpin, L: Write + Unpin> AsyncWrite for LoggedStream<S, L> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let result = Pin::new(&mut this.stream).poll_write(cx, buf);

        if let Poll::Ready(Ok(n)) = result {
            this.log_write(&buf[..n]);
        }

        result
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_close(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let result = Pin::new(&mut this.stream).poll_write_vectored(cx, bufs);

        if let Poll::Ready(Ok(n)) = result {
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

            this.log_write(&bytes);
        }

        result
    }
}

#[cfg(feature = "async")]
impl<S: AsyncRead + Unpin, L: Write + Unpin> AsyncRead for LoggedStream<S, L> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let result = Pin::new(&mut this.stream).poll_read(cx, buf);

        if let Poll::Ready(Ok(n)) = result {
            this.log_read(&buf[..n]);
        }

        result
    }
}

#[cfg(not(feature = "async"))]
impl<S: Stream, L: Write> Stream for LoggedStream<S, L> {}

#[cfg(feature = "async")]
impl<S: Stream, L: Write + Unpin> Stream for LoggedStream<S, L> {}

fn log(mut writer: impl Write, target: &str, data: &[u8]) {
    let _ = match std::str::from_utf8(data) {
        Ok(data) => writeln!(writer, "{}: {:?}", target, data),
        Err(..) => writeln!(writer, "{}:(bytes): {:?}", target, data),
    };
}

pub struct EmptyStream;

impl Write for EmptyStream {
    fn write(&mut self, _: &[u8]) -> Result<usize> {
        Ok(0)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Read for EmptyStream {
    fn read(&mut self, _: &mut [u8]) -> Result<usize> {
        Ok(0)
    }
}

#[cfg(not(feature = "async"))]
impl crate::process::NonBlocking for EmptyStream {
    fn set_non_blocking(&mut self) -> Result<()> {
        Ok(())
    }

    fn set_blocking(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "async")]
impl AsyncWrite for EmptyStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(0))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(0))
    }
}

#[cfg(feature = "async")]
impl AsyncRead for EmptyStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(0))
    }
}

impl Stream for EmptyStream {}
