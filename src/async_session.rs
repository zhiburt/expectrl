use std::{
    io::{self, IoSliceMut},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_lite::{ready, AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite};

use crate::{Error, Found, Needle};

/// Session represents a spawned process and its streams.
/// It controlls process and communication with it.
#[derive(Debug)]
pub struct Stream<S> {
    stream: BufferedStream<S>,
    expect_timeout: Option<Duration>,
}

impl<S> Stream<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream: BufferedStream::new(stream),
            expect_timeout: Some(Duration::from_millis(10000)),
        }
    }

    /// Set the pty session's expect timeout.
    pub fn set_expect_timeout(&mut self, expect_timeout: Option<Duration>) {
        self.expect_timeout = expect_timeout;
    }
}

impl<S> Stream<S> {
    fn swap_stream<NS>(self, stream: NS) -> io::Result<(Stream<NS>, S)> {
        let old_stream = self.stream.stream;
        let buffer = self.stream.buffer;
        let length = self.stream.length;
        Ok((
            Stream {
                stream: BufferedStream {
                    buffer,
                    length,
                    stream,
                },
                expect_timeout: self.expect_timeout,
            },
            old_stream,
        ))
    }
}

impl<S: AsyncRead + Unpin> Stream<S> {
    pub async fn expect<N: Needle>(&mut self, needle: N) -> Result<Found, Error> {
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
                    let end_index = Found::right_most_index(&found);
                    let involved_bytes = data[..end_index].to_vec();
                    self.stream.consume(end_index);
                    return Ok(Found::new(involved_bytes, found));
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
    #[cfg(feature = "async")]
    pub async fn is_matched<E: Needle>(&mut self, needle: E) -> Result<bool, Error> {
        let eof = match futures_lite::future::poll_once(self.stream.fill()).await {
            Some(Ok(n)) => n == 0,
            Some(Err(err)) => return Err(err.into()),
            None => false,
        };

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
    #[cfg(feature = "async")]
    pub async fn check<E: Needle>(&mut self, needle: E) -> Result<Found, Error> {
        let eof = match futures_lite::future::poll_once(self.stream.fill()).await {
            Some(Ok(n)) => n == 0,
            Some(Err(err)) => return Err(err.into()),
            None => false,
        };

        let buf = self.stream.buffer();

        let found = needle.check(buf, eof)?;
        if !found.is_empty() {
            let end_index = Found::right_most_index(&found);
            let involved_bytes = buf[..end_index].to_vec();
            self.stream.consume(end_index);
            return Ok(Found::new(involved_bytes, found));
        }

        if eof {
            return Err(Error::Eof);
        }

        Ok(Found::new(Vec::new(), Vec::new()))
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Stream<S> {
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

impl<S: AsyncRead + Unpin> AsyncRead for Stream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl<S: AsyncRead + Unpin> AsyncBufRead for Stream<S> {
    fn poll_fill_buf(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Pin::new(&mut self.get_mut().stream).poll_fill_buf(cx)
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        Pin::new(&mut self.stream).consume(amt);
    }
}

/// Session represents a spawned process and its streams.
/// It controlls process and communication with it.
#[derive(Debug)]
pub struct BufferedStream<S> {
    stream: S,
    buffer: Vec<u8>,
    length: usize,
}

impl<S> BufferedStream<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            buffer: Vec::new(),
            length: 0,
        }
    }

    pub fn keep(&mut self, buf: &[u8]) {
        self.buffer.extend(buf);
        self.length += buf.len();
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer[..self.length]
    }

    pub fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }
}

impl<S: AsyncRead + Unpin> BufferedStream<S> {
    pub async fn fill(&mut self) -> io::Result<usize> {
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
        self.buffer.drain(..amt);
        self.length -= amt;
    }
}

#[cfg(test)]
mod tests {
    use futures_lite::AsyncWriteExt;

    use crate::Eof;

    use super::*;

    #[test]
    fn test_expect() {
        let buf = b"Hello World".to_vec();
        let cursor = futures_lite::io::Cursor::new(buf);
        let mut stream = Stream::new(cursor);

        futures_lite::future::block_on(async {
            let found = stream.expect("World").await.unwrap();
            assert_eq!(b"Hello ", found.before());
            assert_eq!(vec![b"World"], found.matches());
        });
    }

    #[test]
    fn test_expect_eof() {
        let buf = b"Hello World".to_vec();
        let cursor = futures_lite::io::Cursor::new(buf);
        let mut stream = Stream::new(cursor);

        futures_lite::future::block_on(async {
            let found = stream.expect(Eof).await.unwrap();
            assert_eq!(b"", found.before());
            assert_eq!(vec![b"Hello World"], found.matches());
        });

        let cursor = futures_lite::io::Cursor::new(Vec::new());
        let mut stream = Stream::new(cursor);

        futures_lite::future::block_on(async {
            let err = stream.expect("").await.unwrap_err();
            assert!(matches!(err, Error::Eof));
        });
    }

    #[test]
    fn test_expect_timeout() {
        futures_lite::future::block_on(async {
            let mut stream = Stream::new(NoEofReader::default());
            stream.set_expect_timeout(Some(Duration::from_millis(100)));

            stream.write_all(b"Hello").await.unwrap();

            let err = stream.expect("Hello World").await.unwrap_err();
            assert!(matches!(err, Error::ExpectTimeout));

            stream.write_all(b" World").await.unwrap();
            let found = stream.expect("World").await.unwrap();
            assert_eq!(b"Hello ", found.before());
            assert_eq!(vec![b"World"], found.matches());
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
            assert_eq!(vec![b"World"], found.matches());
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
            assert_eq!(vec![b"World"], found.matches());
        });
    }

    #[derive(Debug, Default)]
    struct NoEofReader {
        data: Vec<u8>,
    }

    impl AsyncWrite for NoEofReader {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.data.extend(buf);
            Poll::Ready(Ok(buf.len()))
        }
    
        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    
        fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncRead for NoEofReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            mut buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            if self.data.is_empty() {
                return Poll::Pending;
            }

            let n = std::io::Write::write(&mut buf, &self.data)?;
            self.data.drain(..n);
            Poll::Ready(Ok(n))
        }
    }
}
