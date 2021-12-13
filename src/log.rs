//! A wrapper of [Session] to log a read/write operations

use crate::{error::Error, session::Session};
use std::{
    io::{self, Write},
    ops::{Deref, DerefMut},
    process::Command,
};

/// A logging wrapper of session
pub struct SessionWithLog {
    inner: Session,
    logger: Option<Box<dyn Write + Send>>,
}

impl SessionWithLog {
    /// Spawn session wrapped with logger.
    ///
    /// See [Session].
    pub fn spawn(cmd: Command) -> Result<Self, Error> {
        let session = Session::spawn(cmd)?;
        Ok(Self {
            inner: session,
            logger: None,
        })
    }

    /// Set a writer for which is used for logging.
    ///
    /// Logger is suppose to be called on all IO operations.
    pub fn set_log<W: Write + Send + 'static>(&mut self, w: W) {
        self.logger = Some(Box::new(w));
    }

    fn log(&mut self, target: &str, data: &[u8]) {
        if let Some(writer) = self.logger.as_mut() {
            let _ = match std::str::from_utf8(data) {
                Ok(s) => writeln!(writer, "{} {:?}", target, s),
                Err(..) => writeln!(writer, "{} (bytes) {:?}", target, data),
            };
        }
    }
}

#[cfg(all(feature = "log", not(feature = "async")))]
impl SessionWithLog {
    pub fn send<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.log("send", s.as_ref().as_bytes());
        self.inner.send(s)
    }

    pub fn send_line<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.log("send_line", s.as_ref().as_bytes());
        self.inner.send_line(s)
    }
}

#[cfg(feature = "async")]
impl SessionWithLog {
    pub async fn send<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.log("send", s.as_ref().as_bytes());
        self.inner.send(s).await
    }

    pub async fn send_line<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.log("send_line", s.as_ref().as_bytes());
        self.inner.send_line(s).await
    }
}

impl Deref for SessionWithLog {
    type Target = Session;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for SessionWithLog {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(not(feature = "async"))]
impl std::io::Write for SessionWithLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.log("write", buf);
        self.deref_mut().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.deref_mut().flush()
    }
}

#[cfg(not(feature = "async"))]
impl std::io::Read for SessionWithLog {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let result = self.deref_mut().read(buf);
        if let Ok(n) = result {
            self.log("read", &buf[..n]);
        }

        result
    }
}

#[cfg(not(feature = "async"))]
impl std::io::BufRead for SessionWithLog {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt)
    }

    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        let size = self.inner.read_until(byte, buf)?;
        self.log("read", &buf[..size]);
        Ok(size)
    }

    fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        let start_index = buf.as_bytes().len();
        let size = self.inner.read_line(buf)?;
        self.log("read", &buf.as_bytes()[start_index..start_index + size]);
        Ok(size)
    }
}

#[cfg(feature = "async")]
impl futures_lite::io::AsyncWrite for SessionWithLog {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.log("write", buf);
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_close(cx)
    }
}

#[cfg(feature = "async")]
impl futures_lite::io::AsyncRead for SessionWithLog {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let result = futures_lite::io::AsyncRead::poll_read(
            std::pin::Pin::new(&mut self.inner), // haven't foudn any better way
            cx,
            buf,
        );

        if let std::task::Poll::Ready(Ok(n)) = result {
            self.log("read", &buf[..n]);
        }

        result
    }
}

#[cfg(feature = "async")]
impl futures_lite::io::AsyncBufRead for SessionWithLog {
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        let this = self.get_mut();
        let proc = std::pin::Pin::new(&mut this.inner);
        proc.poll_fill_buf(cx)
    }

    fn consume(mut self: std::pin::Pin<&mut Self>, amt: usize) {
        std::pin::Pin::new(&mut self.inner).consume(amt);
    }
}

#[cfg(feature = "async")]
impl SessionWithLog {
    /// The function behaives in the same way as [futures_lite::io::AsyncBufReadExt].
    ///
    /// The function is crated as a hack because [futures_lite::io::AsyncBufReadExt] has a default implmentation.
    pub async fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        use futures_lite::AsyncBufReadExt;
        let size = self.inner.read_until(byte, buf).await?;
        self.log("read", &buf[..size]);
        Ok(size)
    }

    /// The function behaives in the same way as [futures_lite::io::AsyncBufReadExt].
    ///
    /// The function is crated as a hack because [futures_lite::io::AsyncBufReadExt] has a default implmentation.
    pub async fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        use futures_lite::AsyncBufReadExt;
        let start_index = buf.as_bytes().len();
        let size = self.inner.read_line(buf).await?;
        self.log("read", &buf.as_bytes()[start_index..start_index + size]);
        Ok(size)
    }
}
