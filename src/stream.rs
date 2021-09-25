//! Module responsible for IO operations of [crate::Session].
//!
//! Module contains [Stream] structure async and sync flow and different one for windows.
//! It also contains a [ReaderWithBuffer] for controlling buffering.

/// Stream represent a IO stream.
#[cfg(not(feature = "async"))]
#[cfg(unix)]
pub type Stream = unix::sync_stream::Stream;

/// Stream represent a IO stream.
#[cfg(feature = "async")]
#[cfg(unix)]
pub type Stream = unix::async_stream::AsyncStream;

/// Stream represent a IO stream.
#[cfg(windows)]
pub type Stream = win::Stream;

#[cfg(unix)]
mod unix {
    #[cfg(not(feature = "async"))]
    pub(super) mod sync_stream {
        use super::super::ReaderWithBuffer;
        use nix::{
            fcntl::{fcntl, FcntlArg, OFlag},
            Result,
        };
        use std::{
            fs::File,
            io::{self, BufRead, BufReader, Read, Write},
            os::unix::prelude::{AsRawFd, RawFd},
        };

        /// Stream represent a IO stream.
        #[derive(Debug)]
        pub struct Stream {
            inner: ptyprocess::stream::Stream,
            reader: BufReader<ReaderWithBuffer<ptyprocess::stream::Stream>>,
        }

        impl Stream {
            /// The function returns a new Stream from a file.
            pub fn new(file: File) -> Self {
                let copy_file = file
                    .try_clone()
                    .expect("It's ok to clone fd as it will be just DUPed");
                let reader = BufReader::new(ReaderWithBuffer::new(
                    ptyprocess::stream::Stream::new(copy_file),
                ));
                let file = ptyprocess::stream::Stream::new(file);

                Self {
                    inner: file,
                    reader,
                }
            }

            /// Try to read in a non-blocking mode.
            ///
            /// It raises io::ErrorKind::WouldBlock if there's nothing to read.
            pub fn try_read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
                let fd = self.inner.as_raw_fd();
                make_non_blocking(fd).map_err(nix_error_to_io)?;

                let result = match self.read(&mut buf) {
                    Ok(n) => Ok(n),
                    Err(err) => Err(err),
                };

                // As file is DUPed changes in one descriptor affects all ones
                // so we need to make blocking file after we finished.
                make_blocking(fd).map_err(nix_error_to_io)?;

                result
            }

            // non-buffered && non-blocking read
            fn try_read_inner(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
                let fd = self.inner.as_raw_fd();
                make_non_blocking(fd).map_err(nix_error_to_io)?;

                let result = match self.reader.get_mut().inner.read(&mut buf) {
                    Ok(n) => Ok(n),
                    Err(err) => Err(err),
                };

                // As file is DUPed changes in one descriptor affects all ones
                // so we need to make blocking file after we finished.
                make_blocking(fd).map_err(nix_error_to_io)?;

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
                let mut buf = [0; 248];
                loop {
                    match self.try_read_inner(&mut buf) {
                        Ok(0) => break Ok(true),
                        Ok(n) => {
                            self.keep_in_buffer(&buf[..n]);
                        }
                        Err(err) if err.kind() == io::ErrorKind::WouldBlock => break Ok(false),
                        Err(err) => break Err(err),
                    }
                }
            }

            pub fn read_available_once(&mut self, buf: &mut [u8]) -> std::io::Result<bool> {
                match self.try_read_inner(buf) {
                    Ok(0) => Ok(true),
                    Ok(n) => {
                        self.keep_in_buffer(&buf[..n]);
                        Ok(false)
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(false),
                    Err(err) => Err(err),
                }
            }

            pub fn get_available(&mut self) -> &[u8] {
                &self.reader.get_mut().buffer
            }

            pub fn consume_from_buffer(&mut self, n: usize) {
                self.reader.get_mut().buffer.drain(..n);
            }

            pub fn keep_in_buffer(&mut self, v: &[u8]) {
                self.reader.get_mut().keep_in_buffer(v);
            }
        }

        impl Write for Stream {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.inner.write(buf)
            }

            fn flush(&mut self) -> io::Result<()> {
                self.inner.flush()
            }

            fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
                self.inner.write_vectored(bufs)
            }
        }

        impl Read for Stream {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.reader.read(buf)
            }
        }

        impl BufRead for Stream {
            fn fill_buf(&mut self) -> io::Result<&[u8]> {
                self.reader.fill_buf()
            }

            fn consume(&mut self, amt: usize) {
                self.reader.consume(amt)
            }
        }

        fn make_non_blocking(fd: RawFd) -> Result<()> {
            _make_non_blocking(fd, true)
        }

        fn make_blocking(fd: RawFd) -> Result<()> {
            _make_non_blocking(fd, false)
        }

        fn _make_non_blocking(fd: RawFd, blocking: bool) -> Result<()> {
            let opt = fcntl(fd, FcntlArg::F_GETFL)?;
            let mut opt = OFlag::from_bits_truncate(opt);
            opt.set(OFlag::O_NONBLOCK, blocking);
            fcntl(fd, FcntlArg::F_SETFL(opt))?;
            Ok(())
        }

        fn nix_error_to_io(err: nix::Error) -> io::Error {
            match err.as_errno() {
                Some(code) => io::Error::from_raw_os_error(code as _),
                None => io::Error::new(
                    io::ErrorKind::Other,
                    "Unexpected error type conversion from nix to io",
                ),
            }
        }
    }

    #[cfg(feature = "async")]
    pub(super) mod async_stream {
        use super::super::ReaderWithBuffer;
        use async_io::Async;
        use futures_lite::{io::BufReader, AsyncBufRead, AsyncRead, AsyncWrite};
        use ptyprocess::stream::Stream;
        use std::{
            fs::File,
            io,
            pin::Pin,
            task::{Context, Poll},
        };

        /// Stream represent a IO stream.
        #[derive(Debug)]
        pub struct AsyncStream {
            inner: Async<Stream>,
            reader: BufReader<ReaderWithBuffer<Async<Stream>>>,
        }

        impl AsyncStream {
            /// The function returns a new Stream from a file.
            pub fn new(file: File) -> Self {
                let cloned = file.try_clone().unwrap();
                let file = Async::new(Stream::new(file)).unwrap();
                let reader = BufReader::new(ReaderWithBuffer::new(
                    Async::new(Stream::new(cloned)).unwrap(),
                ));

                Self {
                    inner: file,
                    reader,
                }
            }

            /// Try to read in a non-blocking mode.
            ///
            /// It raises io::ErrorKind::WouldBlock if there's nothing to read.
            pub async fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                use futures_lite::AsyncReadExt;
                match futures_lite::future::poll_once(self.reader.read(buf)).await {
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
                match futures_lite::future::poll_once(self.reader.get_mut().inner.read(buf)).await {
                    Some(result) => result,
                    None => Err(io::Error::new(io::ErrorKind::WouldBlock, "")),
                }
            }

            pub async fn read_available(&mut self) -> std::io::Result<bool> {
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

            pub async fn read_available_once(&mut self, buf: &mut [u8]) -> std::io::Result<bool> {
                match self.try_read_inner(buf).await {
                    Ok(0) => Ok(true),
                    Ok(n) => {
                        self.keep_in_buffer(&buf[..n]);
                        Ok(false)
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(false),
                    Err(err) => Err(err),
                }
            }

            pub fn get_available(&mut self) -> &[u8] {
                &self.reader.get_mut().buffer
            }

            pub fn consume_from_buffer(&mut self, n: usize) {
                self.reader.get_mut().buffer.drain(..n);
            }

            pub fn keep_in_buffer(&mut self, v: &[u8]) {
                self.reader.get_mut().keep_in_buffer(v);
            }
        }

        impl AsyncWrite for AsyncStream {
            fn poll_write(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &[u8],
            ) -> Poll<io::Result<usize>> {
                <Async<Stream> as AsyncWrite>::poll_write(Pin::new(&mut self.inner), cx, buf)
            }

            fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                <Async<Stream> as AsyncWrite>::poll_flush(Pin::new(&mut self.inner), cx)
            }

            fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                <Async<Stream> as AsyncWrite>::poll_close(Pin::new(&mut self.inner), cx)
            }

            fn poll_write_vectored(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                bufs: &[io::IoSlice<'_>],
            ) -> Poll<io::Result<usize>> {
                <Async<Stream> as AsyncWrite>::poll_write_vectored(
                    Pin::new(&mut self.inner),
                    cx,
                    bufs,
                )
            }
        }

        impl AsyncRead for AsyncStream {
            fn poll_read(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &mut [u8],
            ) -> Poll<io::Result<usize>> {
                Pin::new(&mut self.reader).poll_read(cx, buf)
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
                Pin::new(&mut this.reader).poll_fill_buf(cx)
            }

            fn consume(mut self: Pin<&mut Self>, amt: usize) {
                Pin::new(&mut self.reader).consume(amt)
            }
        }
    }
}

#[cfg(windows)]
mod win {
    use super::ReaderWithBuffer;
    use std::io::{self, BufRead, BufReader, Read, Write};

    /// Stream represent a IO stream.
    #[derive(Debug)]
    pub struct Stream {
        input: conpty::io::PipeWriter,
        output: BufReader<ReaderWithBuffer<conpty::io::PipeReader>>,
    }

    impl Stream {
        /// The function returns a new Stream from a file.
        pub fn new(input: conpty::io::PipeWriter, output: conpty::io::PipeReader) -> Self {
            Self {
                input,
                output: BufReader::new(ReaderWithBuffer::new(output)),
            }
        }

        pub fn try_read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
            self.output.get_mut().get_mut().set_non_blocking_mode()?;

            let result = match self.read(&mut buf) {
                Ok(n) => Ok(n),
                Err(err) => Err(err),
            };

            self.output.get_mut().get_mut().set_blocking_mode()?;

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

        // non-buffered && non-blocking read
        fn try_read_inner(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
            self.output.get_mut().get_mut().set_non_blocking_mode()?;

            let result = match self.output.get_mut().inner.read(&mut buf) {
                Ok(n) => Ok(n),
                Err(err) => Err(err),
            };

            self.output.get_mut().get_mut().set_blocking_mode()?;

            result
        }

        pub fn read_available(&mut self) -> std::io::Result<bool> {
            let mut buf = [0; 248];
            loop {
                match self.try_read_inner(&mut buf) {
                    Ok(0) => break Ok(true),
                    Ok(n) => {
                        self.keep_in_buffer(&buf[..n]);
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => break Ok(false),
                    Err(err) => break Err(err),
                }
            }
        }

        pub fn read_available_once(&mut self, buf: &mut [u8]) -> std::io::Result<bool> {
            match self.try_read_inner(buf) {
                Ok(0) => Ok(true),
                Ok(n) => {
                    self.keep_in_buffer(&buf[..n]);
                    Ok(false)
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(false),
                Err(err) => Err(err),
            }
        }

        pub fn get_available(&mut self) -> &[u8] {
            &self.output.get_mut().buffer
        }

        pub fn consume_from_buffer(&mut self, n: usize) {
            self.output.get_mut().buffer.drain(..n);
        }

        pub fn keep_in_buffer(&mut self, v: &[u8]) {
            self.output.get_mut().keep_in_buffer(v);
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
}

#[derive(Debug)]
struct ReaderWithBuffer<R> {
    inner: R,
    buffer: Vec<u8>,
}

impl<R> ReaderWithBuffer<R> {
    fn keep_in_buffer(&mut self, v: &[u8]) {
        self.buffer.extend(v);
    }

    #[allow(dead_code)]
    fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }
}

#[cfg(not(feature = "async"))]
impl<R: std::io::Read> ReaderWithBuffer<R> {
    fn new(reader: R) -> Self {
        Self {
            inner: reader,
            buffer: Vec::new(),
        }
    }
}

#[cfg(not(feature = "async"))]
impl<R: std::io::Read> std::io::Read for ReaderWithBuffer<R> {
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        // We intentinally try to read from inner buffer in any case
        // because calling code might endlessly save into inner buffer something and actuall read won't be called at all,
        //
        // For example caller code waits before something appear in the buffer,
        // And if its not the read data saved into our buffer.
        // In such a situation we will return a buffer which will never be filled with expected data.
        //
        // As a down side we might lose a error which might be important to caller code.
        if self.buffer.is_empty() {
            self.inner.read(buf)
        } else {
            use std::io::Write;
            let n = buf.write(&self.buffer)?;
            self.buffer.drain(..n);

            self.inner
                .read(&mut buf[n..])
                .map(|n1| {
                    // is it possible that overflow happen?
                    n + n1
                })
                .or(Ok(n))
        }
    }
}

#[cfg(feature = "async")]
impl<R: futures_lite::AsyncRead> ReaderWithBuffer<R> {
    fn new(reader: R) -> Self {
        Self {
            inner: reader,
            buffer: Vec::new(),
        }
    }
}

#[cfg(feature = "async")]
impl<R: futures_lite::AsyncRead + std::marker::Unpin> futures_lite::AsyncRead
    for ReaderWithBuffer<R>
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        mut buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        // see sync version
        if self.buffer.is_empty() {
            std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
        } else {
            use std::io::Write;
            let n = buf.write(&self.buffer)?;
            self.buffer.drain(..n);

            let poll = std::pin::Pin::new(&mut self.inner).poll_read(cx, &mut buf[n..]);
            match poll {
                std::task::Poll::Ready(Ok(n1)) => std::task::Poll::Ready(Ok(n + n1)),
                std::task::Poll::Ready(Err(..)) => std::task::Poll::Ready(Ok(n)),
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        }
    }
}
