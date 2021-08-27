/// Stream represent a IO stream.
#[cfg(not(feature = "async"))]
pub type Stream = sync_stream::Stream;
/// Stream represent a IO stream.
#[cfg(feature = "async")]
pub type Stream = async_stream::AsyncStream;

#[cfg(not(feature = "async"))]
mod sync_stream {
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
        reader: BufReader<ptyprocess::stream::Stream>,
    }

    impl Stream {
        /// The function returns a new Stream from a file.
        pub fn new(file: File) -> Self {
            let copy_file = file
                .try_clone()
                .expect("It's ok to clone fd as it will be just DUPed");
            let reader = BufReader::new(ptyprocess::stream::Stream::new(copy_file));
            let file = ptyprocess::stream::Stream::new(file);

            Self {
                inner: file,
                reader,
            }
        }

        /// Try to read in a non-blocking mode.
        ///
        /// It returns:
        ///     - Ok(None) if there's nothing to read.
        ///     - Ok(Some(n)) an amount of bytes were read.
        ///     - Err(err) an IO error which occured.
        pub fn try_read(&mut self, mut buf: &mut [u8]) -> io::Result<Option<usize>> {
            let fd = self.inner.as_raw_fd();
            make_non_blocking(fd).map_err(nix_error_to_io)?;

            let result = match self.read(&mut buf) {
                Ok(n) => Ok(Some(n)),
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
                Err(err) => Err(err),
            };

            // As file is DUPed changes in one descriptor affects all ones
            // so we need to make blocking file after we finished.
            make_blocking(fd).map_err(nix_error_to_io)?;

            result
        }

        /// Try to read a byte in a non-blocking mode.
        ///
        /// Returns:
        ///     - `None` if there's nothing to read.
        ///     - `Some(None)` on eof.
        ///     - `Some(Some(byte))` on sucessfull call.
        ///
        /// For more information look at [`try_read`].
        ///
        /// [`try_read`]: struct.PtyProcess.html#method.try_read
        pub fn try_read_byte(&mut self) -> io::Result<Option<Option<u8>>> {
            let mut buf = [0; 1];
            match self.try_read(&mut buf)? {
                Some(1) => Ok(Some(Some(buf[0]))),
                Some(0) => Ok(Some(None)),
                None => Ok(None),
                Some(_) => unreachable!(),
            }
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
mod async_stream {
    use async_io::Async;
    use futures_lite::{io::BufReader, AsyncBufRead, AsyncRead, AsyncWrite};
    use ptyprocess::stream::Stream;
    use std::{
        fs::File,
        io::{self, Read},
        pin::Pin,
        task::{Context, Poll},
    };

    /// Stream represent a IO stream.
    #[derive(Debug)]
    pub struct AsyncStream {
        inner: Async<Stream>,
        reader: BufReader<Async<Stream>>,
    }

    impl AsyncStream {
        /// The function returns a new Stream from a file.
        pub fn new(file: File) -> Self {
            let cloned = file.try_clone().unwrap();
            let file = Async::new(Stream::new(file)).unwrap();
            let reader = BufReader::new(Async::new(Stream::new(cloned)).unwrap());

            Self {
                inner: file,
                reader,
            }
        }

        /// Try to read in a non-blocking mode.
        ///
        /// It returns:
        ///     - Ok(None) if there's nothing to read.
        ///     - Ok(Some(n)) an amount of bytes were read.
        ///     - Err(err) an IO error which occured.
        pub async fn try_read(&mut self, mut buf: &mut [u8]) -> io::Result<Option<usize>> {
            // future::poll_once was testing but it doesn't work why?
            // let a = future::poll_once(self.reader.read(buf)).await;
            // match a {
            //     Some(a) => match a {
            //         Ok(n) => Ok(Some(n)),
            //         Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
            //         Err(err) => Err(err),
            //     },
            //     None => Ok(None),
            // }

            // A fd already in a non-blocking mode
            match self.reader.get_mut().as_mut().read(&mut buf) {
                Ok(n) => Ok(Some(n)),
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
                Err(err) => Err(err),
            }
        }

        /// Try to read a byte in a non-blocking mode.
        ///
        /// Returns:
        ///     - `None` if there's nothing to read.
        ///     - `Some(None)` on eof.
        ///     - `Some(Some(byte))` on sucessfull call.
        ///
        /// For more information look at [`try_read`].
        ///
        /// [`try_read`]: struct.PtyProcess.html#method.try_read
        pub async fn try_read_byte(&mut self) -> io::Result<Option<Option<u8>>> {
            let mut buf = [0; 1];
            match self.try_read(&mut buf).await? {
                Some(1) => Ok(Some(Some(buf[0]))),
                Some(0) => Ok(Some(None)),
                None => Ok(None),
                Some(_) => unreachable!(),
            }
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
            <Async<Stream> as AsyncWrite>::poll_write_vectored(Pin::new(&mut self.inner), cx, bufs)
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
