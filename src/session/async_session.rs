//! Module contains an async version of Session structure.

use std::{
    io::{self, IoSliceMut},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_lite::{
    ready, AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt,
};

use crate::{
    process::{Healthcheck, Termios},
    AsyncExpect, Captures, Error, Expect, Needle,
};

/// Session represents a spawned process and its streams.
/// It controlls process and communication with it.
#[derive(Debug)]
pub struct Session<P, S> {
    process: P,
    stream: Stream<S>,
}

// GEt back to the solution where Logger is just dyn Write instead of all these magic with type system.....

impl<P, S> Session<P, S> {
    /// Create a new session.
    pub fn new(process: P, stream: S) -> io::Result<Self> {
        Ok(Self {
            process,
            stream: Stream::new(stream),
        })
    }

    /// Get a reference to original stream.
    pub fn get_stream(&self) -> &S {
        self.stream.as_ref()
    }

    /// Get a mut reference to original stream.
    pub fn get_stream_mut(&mut self) -> &mut S {
        self.stream.as_mut()
    }

    /// Get a reference to a process running program.
    pub fn get_process(&self) -> &P {
        &self.process
    }

    /// Get a mut reference to a process running program.
    pub fn get_process_mut(&mut self) -> &mut P {
        &mut self.process
    }

    /// Set the pty session's expect timeout.
    pub fn set_expect_timeout(&mut self, expect_timeout: Option<Duration>) {
        self.stream.set_expect_timeout(expect_timeout);
    }

    /// Set a expect algorithm to be either gready or lazy.
    ///
    /// Default algorithm is gready.
    ///
    /// See [Session::expect].
    pub fn set_expect_lazy(&mut self, is_lazy: bool) {
        self.stream.expect_lazy = is_lazy;
    }

    pub(crate) fn swap_stream<F: FnOnce(S) -> R, R>(
        mut self,
        new_stream: F,
    ) -> Result<Session<P, R>, Error> {
        let buf = self.stream.get_available().to_owned();

        let stream = self.stream.into_inner();
        let stream = new_stream(stream);
        let mut session = Session::new(self.process, stream)?;
        session.stream.keep(&buf);
        Ok(session)
    }

    /// Verifyes if stream is empty or not.
    pub async fn is_empty(&mut self) -> io::Result<bool>
    where
        S: AsyncRead + Unpin,
    {
        self.stream.is_empty().await
    }
}

impl<P, S> AsyncExpect for Session<P, S>
where
    S: AsyncWrite + AsyncRead + Unpin,
{
    async fn expect<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle,
    {
        match self.stream.expect_lazy {
            true => self.stream.expect_lazy(needle).await,
            false => self.stream.expect_gready(needle).await,
        }
    }

    async fn check<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle,
    {
        self.stream.check(needle).await
    }

    async fn is_matched<N>(&mut self, needle: N) -> Result<bool, Error>
    where
        N: Needle,
    {
        self.stream.is_matched(needle).await
    }

    async fn send<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>,
    {
        self.stream.write_all(buf.as_ref()).await.map_err(Error::IO)
    }

    async fn send_line<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>,
    {
        #[cfg(windows)]
        const LINE_ENDING: &[u8] = b"\r\n";
        #[cfg(not(windows))]
        const LINE_ENDING: &[u8] = b"\n";

        self.stream.write_all(buf.as_ref()).await?;
        self.stream.write_all(LINE_ENDING).await?;

        Ok(())
    }
}

impl<P, S> Healthcheck for Session<P, S>
where
    P: Healthcheck,
{
    type Status = P::Status;

    /// Verifies whether process is still alive.
    fn is_alive(&self) -> io::Result<bool> {
        P::is_alive(self.get_process())
    }

    fn get_status(&self) -> io::Result<Self::Status> {
        P::get_status(self.get_process())
    }
}

impl<P, S> AsyncWrite for Session<P, S>
where
    P: Unpin,
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
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

impl<P, S> AsyncRead for Session<P, S>
where
    P: Unpin,
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl<P, S> AsyncBufRead for Session<P, S>
where
    P: Unpin,
    S: AsyncRead + Unpin,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Pin::new(&mut self.get_mut().stream).poll_fill_buf(cx)
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        Pin::new(&mut self.stream).consume(amt);
    }
}

impl<P, S> Termios for Session<P, S>
where
    P: Termios,
{
    fn is_echo(&self) -> io::Result<bool> {
        P::is_echo(self.get_process())
    }

    fn set_echo(&mut self, on: bool) -> io::Result<bool> {
        P::set_echo(self.get_process_mut(), on)
    }
}

/// Session represents a spawned process and its streams.
/// It controlls process and communication with it.
#[derive(Debug)]
struct Stream<S> {
    stream: BufferedStream<S>,
    expect_timeout: Option<Duration>,
    expect_lazy: bool,
}

impl<S> Stream<S> {
    /// Creates an async IO stream.
    fn new(stream: S) -> Self {
        Self {
            stream: BufferedStream::new(stream),
            expect_timeout: Some(Duration::from_millis(10000)),
            expect_lazy: false,
        }
    }

    /// Returns a reference to original stream.
    fn as_ref(&self) -> &S {
        &self.stream.stream
    }

    /// Returns a mut reference to original stream.
    fn as_mut(&mut self) -> &mut S {
        &mut self.stream.stream
    }

    /// Set the pty session's expect timeout.
    fn set_expect_timeout(&mut self, expect_timeout: Option<Duration>) {
        self.expect_timeout = expect_timeout;
    }

    /// Save a bytes in inner buffer.
    /// They'll be pushed to the end of the buffer.
    fn keep(&mut self, buf: &[u8]) {
        self.stream.keep(buf);
    }

    /// Get an inner buffer.
    fn get_available(&mut self) -> &[u8] {
        self.stream.buffer()
    }

    /// Returns an inner IO stream.
    fn into_inner(self) -> S {
        self.stream.stream
    }
}

impl<S> Stream<S>
where
    S: AsyncRead + Unpin,
{
    async fn expect_gready<N: Needle>(&mut self, needle: N) -> Result<Captures, Error> {
        let expect_timeout = self.expect_timeout;

        let expect_future = async {
            let mut eof = false;
            loop {
                let data = self.stream.buffer();

                let found = Needle::check(&needle, data, eof)?;

                if !found.is_empty() {
                    let end_index = Captures::right_most_index(&found);
                    let involved_bytes = data[..end_index].to_vec();
                    self.stream.consume(end_index);

                    return Ok(Captures::new(involved_bytes, found));
                }

                if eof {
                    return Err(Error::Eof);
                }

                eof = self.stream.fill().await? == 0;
            }
        };

        if let Some(timeout) = expect_timeout {
            let timeout_future = futures_timer::Delay::new(timeout);
            futures_lite::future::or(expect_future, async {
                timeout_future.await;
                Err(Error::ExpectTimeout)
            })
            .await
        } else {
            expect_future.await
        }
    }

    async fn expect_lazy<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle,
    {
        let expect_timeout = self.expect_timeout;
        let expect_future = async {
            // We read by byte to make things as lazy as possible.
            //
            // It's chose is important in using Regex as a Needle.
            // Imagine we have a `\d+` regex.
            // Using such buffer will match string `2` imidiately eventhough right after might be other digit.
            //
            // The second reason is
            // if we wouldn't read by byte EOF indication could be lost.
            // And next blocking std::io::Read operation could be blocked forever.
            //
            // We could read all data available via `read_available` to reduce IO operations,
            // but in such case we would need to keep a EOF indicator internally in stream,
            // which is OK if EOF happens onces, but I am not sure if this is a case.

            let mut checked_length = 0;
            let mut eof = false;
            loop {
                let available = self.stream.buffer();
                let is_buffer_checked = checked_length == available.len();
                if is_buffer_checked {
                    let n = self.stream.fill().await?;
                    eof = n == 0;
                }

                // We intentinally not increase the counter
                // and run check one more time even though the data isn't changed.
                // Because it may be important for custom implementations of Needle.
                let available = self.stream.buffer();
                if checked_length < available.len() {
                    checked_length += 1;
                }

                let data = &available[..checked_length];
                let found = Needle::check(&needle, data, eof)?;
                if !found.is_empty() {
                    let end_index = Captures::right_most_index(&found);
                    let involved_bytes = data[..end_index].to_vec();
                    self.stream.consume(end_index);
                    return Ok(Captures::new(involved_bytes, found));
                }

                if eof {
                    return Err(Error::Eof);
                }
            }
        };

        if let Some(timeout) = expect_timeout {
            let timeout_future = futures_timer::Delay::new(timeout);
            futures_lite::future::or(expect_future, async {
                timeout_future.await;
                Err(Error::ExpectTimeout)
            })
            .await
        } else {
            expect_future.await
        }
    }

    /// Is matched checks if a pattern is matched.
    /// It doesn't consumes bytes from stream.
    async fn is_matched<E: Needle>(&mut self, needle: E) -> Result<bool, Error> {
        let eof = self.try_fill().await?;
        let buf = self.stream.buffer();

        let found = needle.check(buf, eof)?;
        if !found.is_empty() {
            return Ok(true);
        }

        if eof {
            return Err(Error::Eof);
        }

        Ok(false)
    }

    /// Check checks if a pattern is matched.
    /// Returns empty found structure if nothing found.
    async fn check<E>(&mut self, needle: E) -> Result<Captures, Error>
    where
        E: Needle,
    {
        let eof = self.try_fill().await?;

        let buf = self.stream.buffer();
        let found = needle.check(buf, eof)?;
        if !found.is_empty() {
            let end_index = Captures::right_most_index(&found);
            let involved_bytes = buf[..end_index].to_vec();
            self.stream.consume(end_index);
            return Ok(Captures::new(involved_bytes, found));
        }

        if eof {
            return Err(Error::Eof);
        }

        Ok(Captures::new(Vec::new(), Vec::new()))
    }

    /// Verifyes if stream is empty or not.
    async fn is_empty(&mut self) -> io::Result<bool> {
        match futures_lite::future::poll_once(self.read(&mut [])).await {
            Some(Ok(0)) => Ok(true),
            Some(Ok(_)) => Ok(false),
            Some(Err(err)) => Err(err),
            None => Ok(true),
        }
    }

    async fn try_fill(&mut self) -> Result<bool, Error> {
        match futures_lite::future::poll_once(self.stream.fill()).await {
            Some(Ok(n)) => Ok(n == 0),
            Some(Err(err)) => Err(err.into()),
            None => Ok(false),
        }
    }
}

impl<S> AsyncWrite for Stream<S>
where
    S: AsyncWrite + Unpin,
{
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

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.stream.get_mut()).poll_write_vectored(cx, bufs)
    }
}

impl<S> AsyncRead for Stream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        for b in bufs {
            if !b.is_empty() {
                return self.poll_read(cx, b);
            }
        }

        self.poll_read(cx, &mut [])
    }
}

impl<S> AsyncBufRead for Stream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Pin::new(&mut self.get_mut().stream).poll_fill_buf(cx)
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        Pin::new(&mut self.stream).consume(amt);
    }
}

/// Session represents a spawned process and its streams.
/// It controlls process and communication with it.
#[derive(Debug)]
struct BufferedStream<S> {
    stream: S,
    buffer: Vec<u8>,
    length: usize,
}

impl<S> BufferedStream<S> {
    fn new(stream: S) -> Self {
        Self {
            stream,
            buffer: Vec::new(),
            length: 0,
        }
    }

    fn keep(&mut self, buf: &[u8]) {
        self.buffer.extend(buf);
        self.length += buf.len();
    }

    fn buffer(&self) -> &[u8] {
        &self.buffer[..self.length]
    }

    fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }
}

impl<S: AsyncRead + Unpin> BufferedStream<S> {
    async fn fill(&mut self) -> io::Result<usize> {
        let mut buf = [0; 128];
        let n = self.stream.read(&mut buf).await?;
        self.keep(&buf[..n]);
        Ok(n)
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for BufferedStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut rem = ready!(self.as_mut().poll_fill_buf(cx))?;
        let nread = std::io::Read::read(&mut rem, buf)?;
        self.consume(nread);
        Poll::Ready(Ok(nread))
    }

    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        let mut rem = ready!(self.as_mut().poll_fill_buf(cx))?;
        let nread = std::io::Read::read_vectored(&mut rem, bufs)?;
        self.consume(nread);
        Poll::Ready(Ok(nread))
    }
}

impl<S: AsyncRead + Unpin> AsyncBufRead for BufferedStream<S> {
    fn poll_fill_buf(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        if self.buffer.is_empty() {
            let mut buf = [0; 128];
            let n = ready!(Pin::new(&mut self.stream).poll_read(cx, &mut buf))?;
            self.keep(&buf[..n]);
        }

        let buf = self.get_mut().buffer();
        Poll::Ready(Ok(buf))
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        let _ = self.buffer.drain(..amt);
        self.length -= amt;
    }
}

#[cfg(test)]
mod tests {
    use futures_lite::AsyncWriteExt;

    use crate::Eof;

    use super::*;

    #[test]
    fn test_expect_lazy() {
        let buf = b"Hello World".to_vec();
        let cursor = futures_lite::io::Cursor::new(buf);
        let mut stream = Stream::new(cursor);

        futures_lite::future::block_on(async {
            let found = stream.expect_lazy("World").await.unwrap();
            assert_eq!(b"Hello ", found.before());
            assert_eq!(vec![b"World"], found.matches().collect::<Vec<_>>());
        });
    }

    #[test]
    fn test_expect_lazy_eof() {
        let buf = b"Hello World".to_vec();
        let cursor = futures_lite::io::Cursor::new(buf);
        let mut stream = Stream::new(cursor);

        futures_lite::future::block_on(async {
            let found = stream.expect_lazy(Eof).await.unwrap();
            assert_eq!(b"", found.before());
            assert_eq!(vec![b"Hello World"], found.matches().collect::<Vec<_>>());
        });

        let cursor = futures_lite::io::Cursor::new(Vec::new());
        let mut stream = Stream::new(cursor);

        futures_lite::future::block_on(async {
            let err = stream.expect_lazy("").await.unwrap_err();
            assert!(matches!(err, Error::Eof));
        });
    }

    #[test]
    fn test_expect_lazy_timeout() {
        futures_lite::future::block_on(async {
            let mut stream = Stream::new(NoEofReader::default());
            stream.set_expect_timeout(Some(Duration::from_millis(100)));

            stream.write_all(b"Hello").await.unwrap();

            let err = stream.expect_lazy("Hello World").await.unwrap_err();
            assert!(matches!(err, Error::ExpectTimeout));

            stream.write_all(b" World").await.unwrap();
            let found = stream.expect_lazy("World").await.unwrap();
            assert_eq!(b"Hello ", found.before());
            assert_eq!(vec![b"World"], found.matches().collect::<Vec<_>>());
        });
    }

    #[test]
    fn test_expect_gready() {
        let buf = b"Hello World".to_vec();
        let cursor = futures_lite::io::Cursor::new(buf);
        let mut stream = Stream::new(cursor);

        futures_lite::future::block_on(async {
            let found = stream.expect_gready("World").await.unwrap();
            assert_eq!(b"Hello ", found.before());
            assert_eq!(vec![b"World"], found.matches().collect::<Vec<_>>());
        });
    }

    #[test]
    fn test_expect_gready_eof() {
        let buf = b"Hello World".to_vec();
        let cursor = futures_lite::io::Cursor::new(buf);
        let mut stream = Stream::new(cursor);

        futures_lite::future::block_on(async {
            let found = stream.expect_gready(Eof).await.unwrap();
            assert_eq!(b"", found.before());
            assert_eq!(vec![b"Hello World"], found.matches().collect::<Vec<_>>());
        });

        let cursor = futures_lite::io::Cursor::new(Vec::new());
        let mut stream = Stream::new(cursor);

        futures_lite::future::block_on(async {
            let err = stream.expect_gready("").await.unwrap_err();
            assert!(matches!(err, Error::Eof));
        });
    }

    #[test]
    fn test_expect_gready_timeout() {
        futures_lite::future::block_on(async {
            let mut stream = Stream::new(NoEofReader::default());
            stream.set_expect_timeout(Some(Duration::from_millis(100)));

            stream.write_all(b"Hello").await.unwrap();

            let err = stream.expect_gready("Hello World").await.unwrap_err();
            assert!(matches!(err, Error::ExpectTimeout));

            stream.write_all(b" World").await.unwrap();
            let found = stream.expect_gready("World").await.unwrap();
            assert_eq!(b"Hello ", found.before());
            assert_eq!(vec![b"World"], found.matches().collect::<Vec<_>>());
        });
    }

    #[test]
    fn test_check() {
        let buf = b"Hello World".to_vec();
        let cursor = futures_lite::io::Cursor::new(buf);
        let mut stream = Stream::new(cursor);

        futures_lite::future::block_on(async {
            let found = stream.check("World").await.unwrap();
            assert_eq!(b"Hello ", found.before());
            assert_eq!(vec![b"World"], found.matches().collect::<Vec<_>>());
        });
    }

    #[test]
    fn test_is_matched() {
        let mut stream = Stream::new(NoEofReader::default());
        futures_lite::future::block_on(async {
            stream.write_all(b"Hello World").await.unwrap();
            assert!(stream.is_matched("World").await.unwrap());
            assert!(!stream.is_matched("*****").await.unwrap());

            let found = stream.check("World").await.unwrap();
            assert_eq!(b"Hello ", found.before());
            assert_eq!(vec![b"World"], found.matches().collect::<Vec<_>>());
        });
    }

    #[derive(Debug, Default)]
    struct NoEofReader {
        data: Vec<u8>,
    }

    impl AsyncWrite for NoEofReader {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.data.extend(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncRead for NoEofReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            mut buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            if self.data.is_empty() {
                return Poll::Pending;
            }

            let n = std::io::Write::write(&mut buf, &self.data)?;
            let _ = self.data.drain(..n);
            Poll::Ready(Ok(n))
        }
    }
}
