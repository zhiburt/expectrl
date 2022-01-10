use std::{
    io,
    ops::{Deref, DerefMut},
    pin::Pin,
    process::Command,
    task::{Context, Poll},
    time::Duration,
};

use futures_lite::{AsyncBufRead, AsyncRead, AsyncWrite};
use ptyprocess::PtyProcess;

use crate::{
    async_stream::Stream,
    stream::{async_stream::AsyncStream, log::LoggedStream},
    Error, Found, Needle,
};

/// Session represents a spawned process and its streams.
/// It controlls process and communication with it.
#[derive(Debug)]
pub struct Session {
    process: PtyProcess,
    stream: Stream<AsyncStream<LoggedStream<'static, ptyprocess::stream::Stream>>>,
}

// GEt back to the solution where Logger is just dyn Write instead of all these magic with type system.....

impl Session {
    pub fn spawn(command: Command) -> Result<Self, Error> {
        let process = PtyProcess::spawn(command)?;
        let stream = LoggedStream::new(process.get_pty_stream()?, io::sink());
        let stream = AsyncStream::new(stream)?;
        let stream = Stream::new(stream);
        Ok(Self { process, stream })
    }

    /// Set logger.
    pub fn set_log<W: io::Write + 'static>(&mut self, logger: W) {
        self.stream.get_mut().get_mut().set_logger(logger);
    }

    /// Set the pty session's expect timeout.
    pub fn set_expect_timeout(&mut self, expect_timeout: Option<Duration>) {
        self.stream.set_expect_timeout(expect_timeout);
    }

    pub async fn expect<N: Needle>(&mut self, needle: N) -> Result<Found, Error> {
        self.stream.expect(needle).await
    }

    /// Is matched checks if a pattern is matched.
    /// It doesn't consumes bytes from stream.
    pub async fn is_matched<E: Needle>(&mut self, needle: E) -> Result<bool, Error> {
        self.stream.is_matched(needle).await
    }

    /// Check checks if a pattern is matched.
    /// Returns empty found structure if nothing found.
    ///
    /// Is a non blocking version of [Session::expect].
    /// But its strategy of matching is different from it.
    /// It makes search agains all bytes available.
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// let mut p = expectrl::spawn("echo 123").unwrap();
    /// // wait to guarantee that check will successed (most likely)
    /// std::thread::sleep(std::time::Duration::from_secs(1));
    /// let m = p.check(expectrl::Regex("\\d+")).await.unwrap();
    /// assert_eq!(m.first(), b"123");
    /// # });
    /// ```
    pub async fn check<E: Needle>(&mut self, needle: E) -> Result<Found, Error> {
        self.stream.check(needle).await
    }
}

impl Deref for Session {
    type Target = PtyProcess;

    fn deref(&self) -> &Self::Target {
        &self.process
    }
}

impl DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.process
    }
}

impl AsyncWrite for Session {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.stream.get_mut()).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.stream.get_mut()).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.stream.get_mut()).poll_close(cx)
    }
}

impl AsyncRead for Session {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncBufRead for Session {
    fn poll_fill_buf(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Pin::new(&mut self.get_mut().stream).poll_fill_buf(cx)
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        Pin::new(&mut self.stream).consume(amt);
    }
}
