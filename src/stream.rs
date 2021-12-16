//! Module responsible for IO operations of [crate::Session].
//!
//! Module contains [Stream] structure async and sync flow and different one for windows.
//! It also contains a [ReaderWithBuffer] for controlling buffering.

use std::io;

#[cfg(feature = "async")]
#[cfg(unix)]
use std::os::unix::prelude::AsRawFd;
#[cfg(feature = "async")]
use std::pin::Pin;
#[cfg(feature = "async")]
use std::task::{Context, Poll};
use std::{
    io::{Read, Write},
    ops::{Deref, DerefMut},
};

#[cfg(feature = "async")]
use futures_lite::AsyncWrite;
#[cfg(feature = "async")]
use futures_lite::{AsyncBufRead, AsyncRead};

#[cfg(feature = "async")]
use async_stream::AsyncStream;
#[cfg(feature = "async")]
use non_blocking_reader::TryReader;
#[cfg(not(feature = "async"))]
use non_blocking_reader::TryReader;

use crate::process::NonBlocking;

#[cfg(not(feature = "async"))]
type Reader<S> = TryReader<S>;

#[cfg(feature = "async")]
type Reader<S> = TryReader<AsyncStream<S>>;

#[derive(Debug)]
pub struct TryStream<S: Read> {
    stream: Reader<S>,
}

#[cfg(not(feature = "async"))]
impl<S: Write + Read + NonBlocking> TryStream<S> {
    /// The function returns a new Stream from a file.
    pub fn new(stream: S) -> io::Result<Self> {
        Ok(Self {
            stream: TryReader::new(stream)?,
        })
    }
}

#[cfg(feature = "async")]
impl<S: Write + Read + AsRawFd> TryStream<S> {
    /// The function returns a new Stream from a file.
    pub fn new(stream: S) -> io::Result<Self> {
        Ok(Self {
            stream: TryReader::new(AsyncStream::new(stream)?)?,
        })
    }
}

impl<P, S: Read> TryStream<S> {
    fn from_stream<N: Read>(&mut self, stream: N) -> io::Result<TryStream<N>> {
        self.stream.flush_in_buffer();
        let buffer = self.stream.get_available();
        let mut stream = TryStream::new(stream)?;
        stream.keep_in_buffer(buffer);
        Ok(stream)
    }
}

#[cfg(not(feature = "async"))]
impl<S: Read + Write> Write for TryStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.get_mut().flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.stream.get_mut().write_vectored(bufs)
    }
}

#[cfg(feature = "async")]
impl<S: Write + Read> AsyncWrite for TryStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream.get_mut()).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream.get_mut()).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream.get_mut()).poll_close(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream.get_mut()).poll_write_vectored(cx, bufs)
    }
}

#[cfg(feature = "async")]
impl<S: Read + Unpin> AsyncRead for TryStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(self.stream.deref_mut()).poll_read(cx, buf)
    }
}

#[cfg(feature = "async")]
impl<S: Read + Unpin> AsyncBufRead for TryStream<S> {
    fn poll_fill_buf<'a>(
        self: Pin<&'a mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<&'a [u8]>> {
        // pin_project is used only for this function.
        // the solution was found in the original implementation of BufReader.
        let this = self.get_mut();
        Pin::new(this.stream.deref_mut()).poll_fill_buf(cx)
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        Pin::new(self.stream.deref_mut()).consume(amt)
    }
}

#[cfg(not(feature = "async"))]
impl<S: Read> Deref for TryStream<S> {
    type Target = TryReader<S>;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

#[cfg(feature = "async")]
impl<S: Read> Deref for TryStream<S> {
    type Target = TryReader<AsyncStream<S>>;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<S: Read> DerefMut for TryStream<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

#[cfg(feature = "async")]
pub(super) mod async_stream {
    use std::{
        io::{self, Read, Write},
        os::unix::prelude::AsRawFd,
        pin::Pin,
        task::{Context, Poll},
    };

    use async_io::Async;
    use futures_lite::{AsyncRead, AsyncWrite};

    /// Stream represent a IO stream.
    #[derive(Debug)]
    pub struct AsyncStream<S> {
        stream: Async<S>,
    }

    impl<S: AsRawFd> AsyncStream<S> {
        /// The function returns a new Stream from a file.
        pub fn new(stream: S) -> io::Result<Self> {
            let stream = Async::new(stream)?;
            Ok(Self { stream })
        }
    }

    impl<S: Write> AsyncWrite for AsyncStream<S> {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.stream).poll_write(cx, buf)
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

    impl<S: Read> AsyncRead for AsyncStream<S> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.stream).poll_read(cx, buf)
        }
    }
}

#[cfg(not(feature = "async"))]
pub mod non_blocking_reader {
    use super::NonBlocking;

    use std::io::{self, BufRead, BufReader, Read};
    use std::ops::{Deref, DerefMut};

    #[derive(Debug)]
    pub struct TryReader<R> {
        inner: ControlledReader<R>,
    }

    impl<R: Read + NonBlocking> TryReader<R> {
        pub fn new(reader: R) -> io::Result<Self> {
            Ok(Self {
                inner: ControlledReader::new(reader),
            })
        }

        /// Try to read in a non-blocking mode.
        ///
        /// It raises io::ErrorKind::WouldBlock if there's nothing to read.
        pub fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.inner.get_mut().set_non_blocking()?;

            let result = match self.inner.read(buf) {
                Ok(n) => Ok(n),
                Err(err) => Err(err),
            };

            // As file is DUPed changes in one descriptor affects all ones
            // so we need to make blocking file after we finished.
            self.inner.get_mut().set_blocking()?;

            result
        }

        pub fn is_empty(&mut self) -> io::Result<bool> {
            match self.try_read(&mut []) {
                Ok(0) => Ok(true),
                Ok(_) => Ok(false),
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(true),
                Err(err) => Err(err),
            }
        }

        pub fn read_available(&mut self) -> std::io::Result<bool> {
            self.inner.flush_in_buffer();

            let mut buf = [0; 248];
            loop {
                match self.try_read_inner(&mut buf) {
                    Ok(0) => break Ok(true),
                    Ok(n) => {
                        self.inner.keep_in_buffer(&buf[..n]);
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => break Ok(false),
                    Err(err) => break Err(err),
                }
            }
        }

        pub fn read_available_once(&mut self, buf: &mut [u8]) -> std::io::Result<Option<usize>> {
            self.inner.flush_in_buffer();

            match self.try_read_inner(buf) {
                Ok(0) => Ok(Some(0)),
                Ok(n) => {
                    self.inner.keep_in_buffer(&buf[..n]);
                    Ok(Some(n))
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
                Err(err) => Err(err),
            }
        }

        // non-buffered && non-blocking read
        fn try_read_inner(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.inner.get_mut().set_non_blocking()?;

            let result = match self.inner.get_mut().read(buf) {
                Ok(n) => Ok(n),
                Err(err) => Err(err),
            };

            // As file is DUPed changes in one descriptor affects all ones
            // so we need to make blocking file after we finished.
            self.inner.get_mut().set_blocking()?;

            result
        }
    }

    impl<R> Deref for TryReader<R> {
        type Target = ControlledReader<R>;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl<R> DerefMut for TryReader<R> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }

    #[derive(Debug)]
    pub struct ControlledReader<R> {
        inner: BufReader<BufferedReader<R>>,
    }

    impl<R: Read> ControlledReader<R> {
        pub fn new(reader: R) -> Self {
            Self {
                inner: BufReader::new(BufferedReader::new(reader)),
            }
        }

        pub fn keep_in_buffer(&mut self, v: &[u8]) {
            self.inner.get_mut().buffer.extend(v);
        }

        pub fn get_mut(&mut self) -> &mut R {
            &mut self.inner.get_mut().inner
        }

        pub fn get_available(&mut self) -> &[u8] {
            &self.inner.get_ref().buffer
        }

        pub fn consume_available(&mut self, n: usize) {
            self.inner.get_mut().buffer.drain(..n);
        }

        pub fn flush_in_buffer(&mut self) {
            // Because we have 2 buffered streams there might appear inconsistancy
            // in read operations and the data which was via `keep_in_buffer` function.
            //
            // To eliminate it we move BufReader buffer to our buffer.
            let b = self.inner.buffer().to_vec();
            self.inner.consume(b.len());
            self.keep_in_buffer(&b);
        }
    }

    impl<R: Read> Read for ControlledReader<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.inner.read(buf)
        }
    }

    impl<R: Read> BufRead for ControlledReader<R> {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            self.inner.fill_buf()
        }

        fn consume(&mut self, amt: usize) {
            self.inner.consume(amt)
        }
    }

    #[derive(Debug)]
    struct BufferedReader<R> {
        inner: R,
        buffer: Vec<u8>,
    }

    impl<R> BufferedReader<R> {
        fn new(reader: R) -> Self {
            Self {
                inner: reader,
                buffer: Vec::new(),
            }
        }
    }

    impl<R: std::io::Read> std::io::Read for BufferedReader<R> {
        fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
            if self.buffer.is_empty() {
                self.inner.read(buf)
            } else {
                use std::io::Write;
                let n = buf.write(&self.buffer)?;
                self.buffer.drain(..n);
                Ok(n)
            }
        }
    }
}

#[cfg(feature = "async")]
pub mod non_blocking_reader {
    use std::{
        io::{self, Result},
        marker::Unpin,
        ops::{Deref, DerefMut},
        pin::Pin,
        task::{Context, Poll},
    };

    use futures_lite::{io::BufReader, AsyncBufRead, AsyncBufReadExt, AsyncRead};

    #[derive(Debug)]
    pub struct TryReader<R: AsyncRead + Unpin> {
        inner: ControlledReader<R>,
    }

    impl<R: AsyncRead + Unpin> TryReader<R> {
        pub fn new(reader: R) -> io::Result<Self> {
            Ok(Self {
                inner: ControlledReader::new(reader)?,
            })
        }

        /// Try to read in a non-blocking mode.
        ///
        /// It raises io::ErrorKind::WouldBlock if there's nothing to read.
        pub async fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            use futures_lite::AsyncReadExt;
            match futures_lite::future::poll_once(self.inner.read(buf)).await {
                Some(result) => result,
                None => Err(io::Error::new(io::ErrorKind::WouldBlock, "")),
            }
        }

        pub async fn is_empty(&mut self) -> io::Result<bool> {
            match self.try_read(&mut []).await {
                Ok(0) => Ok(true),
                Ok(_) => Ok(false),
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(true),
                Err(err) => Err(err),
            }
        }

        // non-buffered && non-blocking read
        async fn try_read_inner(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            use futures_lite::AsyncReadExt;
            match futures_lite::future::poll_once(self.inner.get_mut().read(buf)).await {
                Some(result) => result,
                None => Err(io::Error::new(io::ErrorKind::WouldBlock, "")),
            }
        }

        pub async fn read_available(&mut self) -> std::io::Result<bool> {
            self.inner.flush_in_buffer();

            let mut buf = [0; 248];
            loop {
                match self.try_read_inner(&mut buf).await {
                    Ok(0) => break Ok(true),
                    Ok(n) => {
                        self.keep_in_buffer(&buf[..n]);
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => break Ok(false),
                    Err(err) => break Err(err),
                }
            }
        }

        pub async fn read_available_once(
            &mut self,
            buf: &mut [u8],
        ) -> std::io::Result<Option<usize>> {
            self.flush_in_buffer();

            match self.try_read_inner(buf).await {
                Ok(0) => Ok(Some(0)),
                Ok(n) => {
                    self.keep_in_buffer(&buf[..n]);
                    Ok(Some(n))
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
                Err(err) => Err(err),
            }
        }
    }

    impl<R: AsyncRead + Unpin> Deref for TryReader<R> {
        type Target = ControlledReader<R>;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl<R: AsyncRead + Unpin> DerefMut for TryReader<R> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }

    #[derive(Debug)]
    pub struct ControlledReader<R: AsyncRead + Unpin> {
        inner: BufReader<BufferedReader<R>>,
    }

    impl<R: AsyncRead + Unpin> ControlledReader<R> {
        pub fn new(reader: R) -> io::Result<Self> {
            Ok(Self {
                inner: BufReader::new(BufferedReader::new(reader)),
            })
        }

        pub fn get_mut(&mut self) -> &mut R {
            &mut self.inner.get_mut().inner
        }

        pub fn keep_in_buffer(&mut self, v: &[u8]) {
            self.inner.get_mut().buffer.extend(v);
        }

        pub fn get_available(&mut self) -> &[u8] {
            &self.inner.get_ref().buffer
        }

        pub fn consume_from_buffer(&mut self, n: usize) {
            self.inner.get_mut().buffer.drain(..n);
        }

        pub fn flush_in_buffer(&mut self) {
            // Because we have 2 buffered streams there might appear inconsistancy
            // in read operations and the data which was via `keep_in_buffer` function.
            //
            // To eliminate it we move BufReader buffer to our buffer.
            let b = self.inner.buffer().to_vec();
            self.inner.consume(b.len());
            self.keep_in_buffer(&b);
        }
    }

    impl<R: AsyncRead + Unpin> AsyncRead for ControlledReader<R> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.inner).poll_read(cx, buf)
        }
    }

    impl<R: AsyncRead + Unpin> AsyncBufRead for ControlledReader<R> {
        fn poll_fill_buf<'a>(
            self: Pin<&'a mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<io::Result<&'a [u8]>> {
            // pin_project is used only for this function.
            // the solution was found in the original implementation of BufReader.
            let this = self.get_mut();
            Pin::new(&mut this.inner).poll_fill_buf(cx)
        }

        fn consume(mut self: Pin<&mut Self>, amt: usize) {
            Pin::new(&mut self.inner).consume(amt)
        }
    }

    #[derive(Debug)]
    struct BufferedReader<R> {
        inner: R,
        buffer: Vec<u8>,
    }

    impl<R> BufferedReader<R> {
        fn new(reader: R) -> Self {
            Self {
                inner: reader,
                buffer: Vec::new(),
            }
        }
    }

    #[cfg(feature = "async")]
    impl<R: AsyncRead + Unpin> AsyncRead for BufferedReader<R> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            mut buf: &mut [u8],
        ) -> Poll<Result<usize>> {
            if self.buffer.is_empty() {
                Pin::new(&mut self.inner).poll_read(cx, buf)
            } else {
                use std::io::Write;
                let n = buf.write(&self.buffer)?;
                self.buffer.drain(..n);
                Poll::Ready(Ok(n))
            }
        }
    }
}
