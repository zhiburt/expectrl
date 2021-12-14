//! Module responsible for IO operations of [crate::Session].
//!
//! Module contains [Stream] structure async and sync flow and different one for windows.
//! It also contains a [ReaderWithBuffer] for controlling buffering.

/// Stream represent a IO stream.
#[cfg(not(feature = "async"))]
pub type Stream = sync_stream::Stream;

/// Stream represent a IO stream.
#[cfg(feature = "async")]
#[cfg(unix)]
pub type Stream = async_stream::AsyncStream;

#[cfg(not(feature = "async"))]
pub(super) mod sync_stream {
    use std::{
        fs::File,
        io::{self, BufRead, Read, Write},
    };

    #[cfg(unix)]
    use std::os::unix::prelude::{AsRawFd, RawFd};

    use super::non_blocking_reader::{NonBlocking, TryReader};

    #[cfg(unix)]
    type StreamReader = ptyprocess::stream::Stream;
    #[cfg(unix)]
    type StreamWriter = ptyprocess::stream::Stream;

    #[cfg(windows)]
    type StreamReader = conpty::io::PipeReader;
    #[cfg(windows)]
    type StreamWriter = conpty::io::PipeWriter;

    /// Stream represent a IO stream.
    #[derive(Debug)]
    pub struct Stream {
        input: StreamWriter,
        output: TryReader<StreamReader>,
    }

    #[cfg(windows)]
    impl Stream {
        /// The function returns a new Stream from a file.
        pub fn new(input: conpty::io::PipeWriter, output: conpty::io::PipeReader) -> Self {
            Self {
                input,
                output: TryReader::new(output),
            }
        }
    }

    #[cfg(unix)]
    impl Stream {
        /// The function returns a new Stream from a file.
        pub fn new(file: File) -> io::Result<Self> {
            let copy_file = file.try_clone()?;
            let reader = TryReader::new(ptyprocess::stream::Stream::new(copy_file))?;
            let file = ptyprocess::stream::Stream::new(file);

            Ok(Self {
                input: file,
                output: reader,
            })
        }
    }

    impl Stream {
        /// Try to read in a non-blocking mode.
        ///
        /// It raises io::ErrorKind::WouldBlock if there's nothing to read.
        pub fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.output.try_read(buf)
        }

        pub fn read_available(&mut self) -> std::io::Result<bool> {
            self.output.read_available()
        }

        pub fn read_available_once(&mut self, buf: &mut [u8]) -> std::io::Result<Option<usize>> {
            self.output.read_available_once(buf)
        }

        pub fn is_empty(&mut self) -> io::Result<bool> {
            self.output.is_empty()
        }

        pub fn get_available(&mut self) -> &[u8] {
            self.output.buffer()
        }

        pub fn consume_available(&mut self, n: usize) {
            self.output.consume_from_buffer(n)
        }
    }

    impl Write for Stream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.input.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.input.flush()
        }

        fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
            self.input.write_vectored(bufs)
        }
    }

    impl Read for Stream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.output.read(buf)
        }
    }

    impl BufRead for Stream {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            self.output.fill_buf()
        }

        fn consume(&mut self, amt: usize) {
            self.output.consume(amt)
        }
    }

    #[cfg(unix)]
    impl<A: AsRawFd> NonBlocking for A {
        fn set_non_blocking(&mut self) -> io::Result<()> {
            let fd = self.as_raw_fd();
            _make_non_blocking(fd, true)
        }

        fn set_blocking(&mut self) -> io::Result<()> {
            let fd = self.as_raw_fd();
            _make_non_blocking(fd, false)
        }
    }

    #[cfg(unix)]
    fn _make_non_blocking(fd: RawFd, blocking: bool) -> io::Result<()> {
        use nix::fcntl::{fcntl, FcntlArg, OFlag};

        let opt = fcntl(fd, FcntlArg::F_GETFL).map_err(nix_error_to_io)?;
        let mut opt = OFlag::from_bits_truncate(opt);
        opt.set(OFlag::O_NONBLOCK, blocking);
        fcntl(fd, FcntlArg::F_SETFL(opt)).map_err(nix_error_to_io)?;
        Ok(())
    }

    #[cfg(unix)]
    fn nix_error_to_io(err: nix::Error) -> io::Error {
        match err.as_errno() {
            Some(code) => io::Error::from_raw_os_error(code as _),
            None => io::Error::new(
                io::ErrorKind::Other,
                "Unexpected error type conversion from nix to io",
            ),
        }
    }

    #[cfg(windows)]
    impl NonBlocking for PipeReader {
        fn set_non_blocking(&mut self) -> io::Result<()> {
            self.set_non_blocking_mode()
        }

        fn set_blocking(&mut self) -> io::Result<()> {
            self.set_blocking_mode()
        }
    }
}

#[cfg(feature = "async")]
pub(super) mod async_stream {
    use async_io::Async;
    use futures_lite::{AsyncBufRead, AsyncRead, AsyncWrite};
    use ptyprocess::stream::Stream;
    use std::{
        fs::File,
        io,
        ops::DerefMut,
        pin::Pin,
        task::{Context, Poll},
    };

    use super::non_blocking_reader::TryReader;

    /// Stream represent a IO stream.
    #[derive(Debug)]
    pub struct AsyncStream {
        inner: Async<File>,
        reader: TryReader<Stream>,
    }

    impl AsyncStream {
        /// The function returns a new Stream from a file.
        pub fn new(file: File) -> io::Result<Self> {
            let cloned = file.try_clone().unwrap();
            let reader = TryReader::new(Stream::new(file))?;
            let file = Async::new(cloned).unwrap();

            Ok(Self {
                inner: file,
                reader,
            })
        }

        /// Try to read in a non-blocking mode.
        ///
        /// It raises io::ErrorKind::WouldBlock if there's nothing to read.
        pub async fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.reader.try_read(buf).await
        }

        pub async fn is_empty(&mut self) -> io::Result<bool> {
            self.reader.is_empty().await
        }

        pub async fn read_available(&mut self) -> std::io::Result<bool> {
            self.reader.read_available().await
        }

        pub async fn read_available_once(
            &mut self,
            buf: &mut [u8],
        ) -> std::io::Result<Option<usize>> {
            self.reader.read_available_once(buf).await
        }

        pub fn get_available(&mut self) -> &[u8] {
            self.reader.available()
        }

        pub fn consume_from_buffer(&mut self, n: usize) {
            self.reader.consume_from_buffer(n);
        }
    }

    impl AsyncWrite for AsyncStream {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            <Async<File> as AsyncWrite>::poll_write(Pin::new(&mut self.inner), cx, buf)
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            <Async<File> as AsyncWrite>::poll_flush(Pin::new(&mut self.inner), cx)
        }

        fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            <Async<File> as AsyncWrite>::poll_close(Pin::new(&mut self.inner), cx)
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[io::IoSlice<'_>],
        ) -> Poll<io::Result<usize>> {
            <Async<File> as AsyncWrite>::poll_write_vectored(Pin::new(&mut self.inner), cx, bufs)
        }
    }

    impl AsyncRead for AsyncStream {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(self.reader.deref_mut()).poll_read(cx, buf)
        }
    }

    impl AsyncBufRead for AsyncStream {
        fn poll_fill_buf<'a>(
            self: Pin<&'a mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<io::Result<&'a [u8]>> {
            // pin_project is used only for this function.
            // the solution was found in the original implementation of BufReader.
            let this = self.get_mut();
            Pin::new(this.reader.deref_mut()).poll_fill_buf(cx)
        }

        fn consume(mut self: Pin<&mut Self>, amt: usize) {
            Pin::new(self.reader.deref_mut()).consume(amt)
        }
    }
}

#[cfg(not(feature = "async"))]
pub mod non_blocking_reader {
    use super::reader::ControlledReader;

    use std::{
        io::{self, Read},
        ops::{Deref, DerefMut},
    };

    #[derive(Debug)]
    pub struct TryReader<R> {
        inner: ControlledReader<R>,
    }

    pub trait NonBlocking {
        fn set_non_blocking(&mut self) -> io::Result<()>;
        fn set_blocking(&mut self) -> io::Result<()>;
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
}

#[cfg(feature = "async")]
pub mod non_blocking_reader {
    use super::reader::ControlledReader;

    use std::{
        io::{self, Read},
        ops::{Deref, DerefMut},
        os::unix::prelude::AsRawFd,
    };

    #[derive(Debug)]
    pub struct TryReader<R: Read> {
        inner: ControlledReader<R>,
    }

    impl<R: Read + AsRawFd> TryReader<R> {
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

    impl<R: Read> Deref for TryReader<R> {
        type Target = ControlledReader<R>;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl<R: Read> DerefMut for TryReader<R> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }
}

#[cfg(not(feature = "async"))]
pub mod reader {
    use std::io::{self, BufRead, BufReader, Read};

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

        pub fn buffer(&mut self) -> &[u8] {
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
pub mod reader {
    use std::{
        io::{self, Read, Result},
        marker::Unpin,
        os::unix::prelude::AsRawFd,
        pin::Pin,
        task::{Context, Poll},
    };

    use async_io::Async;
    use futures_lite::{io::BufReader, AsyncBufRead, AsyncBufReadExt, AsyncRead};

    #[derive(Debug)]
    pub struct ControlledReader<R: Read> {
        inner: BufReader<BufferedReader<Async<R>>>,
    }

    impl<R: Read + AsRawFd> ControlledReader<R> {
        pub fn new(reader: R) -> io::Result<Self> {
            Ok(Self {
                inner: BufReader::new(BufferedReader::new(Async::new(reader)?)),
            })
        }

        pub fn get_mut(&mut self) -> &mut Async<R> {
            &mut self.inner.get_mut().inner
        }

        pub fn keep_in_buffer(&mut self, v: &[u8]) {
            self.inner.get_mut().buffer.extend(v);
        }

        pub fn available(&mut self) -> &[u8] {
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

    impl<R: Read> AsyncRead for ControlledReader<R> {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.inner).poll_read(cx, buf)
        }
    }

    impl<R: Read> AsyncBufRead for ControlledReader<R> {
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
