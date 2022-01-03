//! Module contains a Session structure.

use crate::{
    control_code::ControlCode,
    error::Error,
    log,
    needle::Needle,
    process::{Process, Stream},
    stream::TryStream,
    Found, Expect,
};
use std::{
    convert::TryInto,
    io::{self, BufRead, Read, Write},
    ops::{Deref, DerefMut},
    time::{self, Duration},
};

#[cfg(all(unix, feature = "async"))]
use futures_lite::AsyncWriteExt;

/// Session represents a spawned process and its streams.
/// It controlls process and communication with it.
#[derive(Debug)]
pub struct Session<P, S: Stream> {
    proc: P,
    stream: TryStream<S>,
    expect_timeout: Option<Duration>,
}

impl<P> Session<P, P::Stream>
where
    P: Process,
    P::Stream: Stream,
{
    /// Create a session
    pub fn from_process(mut process: P) -> io::Result<Self> {
        let stream = process.stream()?;
        Self::new(process, stream)
    }
}

impl<P, S: Stream> Session<P, S> {
    /// Create a session
    pub fn new(process: P, stream: S) -> io::Result<Self> {
        let stream = TryStream::new(stream)?;
        Ok(Self {
            proc: process,
            stream,
            expect_timeout: Some(Duration::from_millis(10000)),
        })
    }

    /// Set the pty session's expect timeout.
    pub fn set_expect_timeout(&mut self, expect_timeout: Option<Duration>) {
        self.expect_timeout = expect_timeout;
    }

    #[cfg(not(feature = "async"))]
    pub fn with_log<W: Write>(
        self,
        logger: W,
    ) -> Result<Session<P, log::LoggedStream<S, W>>, Error> {
        let (session, old) = self.swap_stream(log::EmptyStream)?;
        let stream = log::LoggedStream::new(old, logger);
        let (session, _) = session.swap_stream(stream)?;

        Ok(session)
    }

    #[cfg(feature = "async")]
    pub fn with_log<W: Write + Unpin>(
        self,
        logger: W,
    ) -> Result<Session<log::LoggedStream<S, W>>, Error> {
        let (session, old) = self.swap_stream(log::EmptyStream)?;
        let stream = log::LoggedStream::new(old, logger);
        let (session, _) = session.swap_stream(stream)?;

        Ok(session)
    }

    pub(crate) fn swap_stream<N: Stream>(self, stream: N) -> io::Result<(Session<P, N>, S)> {
        let (stream, old) = self.stream.swap_stream(stream)?;
        Ok((
            Session {
                proc: self.proc,
                stream,
                expect_timeout: self.expect_timeout,
            },
            old,
        ))
    }
}

impl<P, S: Stream> Expect for Session<P, S> {
    fn expect<E: Needle>(&mut self, expect: E) -> Result<Found, Error> {
        let mut checking_data_length = 0;
        let mut eof = false;
        let start = time::Instant::now();
        loop {
            let mut available = self.stream.get_available();
            if checking_data_length == available.len() {
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
                eof = self.stream.read_available_once(&mut [0; 1])? == Some(0);
                available = self.stream.get_available();
            }

            // We intentinally not increase the counter
            // and run check one more time even though the data isn't changed.
            // Because it may be important for custom implementations of Needle.
            if checking_data_length < available.len() {
                checking_data_length += 1;
            }

            let data = &available[..checking_data_length];

            let found = expect.check(data, eof)?;
            if !found.is_empty() {
                let end_index = Found::right_most_index(&found);
                let involved_bytes = data[..end_index].to_vec();
                self.stream.consume_available(end_index);
                return Ok(Found::new(involved_bytes, found));
            }

            if eof {
                return Err(Error::Eof);
            }

            if let Some(timeout) = self.expect_timeout {
                if start.elapsed() > timeout {
                    return Err(Error::ExpectTimeout);
                }
            }
        }
    }

    fn check<E: Needle>(&mut self, needle: E) -> Result<Found, Error> {
        let eof = self.stream.read_available()?;
        let buf = self.stream.get_available();

        let found = needle.check(buf, eof)?;
        if !found.is_empty() {
            let end_index = Found::right_most_index(&found);
            let involved_bytes = buf[..end_index].to_vec();
            self.stream.consume_available(end_index);
            return Ok(Found::new(involved_bytes, found));
        }

        if eof {
            return Err(Error::Eof);
        }

        Ok(Found::new(Vec::new(), Vec::new()))
    }

    fn is_matched<E: Needle>(&mut self, needle: E) -> Result<bool, Error> {
        let eof = self.stream.read_available()?;
        let buf = self.stream.get_available();

        let found = needle.check(buf, eof)?;
        if !found.is_empty() {
            return Ok(true);
        }

        if eof {
            return Err(Error::Eof);
        }

        Ok(false)
    }

    fn send(&mut self, s: impl AsRef<str>) -> io::Result<()> {
        self.stream.write_all(s.as_ref().as_bytes())
    }

    fn send_line(&mut self, s: impl AsRef<str>) -> io::Result<()> {
        #[cfg(windows)]
        {
            // win32 has writefilegather function which could be used as write_vectored but it asyncronos which may involve some issue?
            // https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-writefilegather

            const LINE_ENDING: &[u8] = b"\r\n";
            let _ = self.write_all(s.as_ref().as_bytes())?;
            let _ = self.write_all(LINE_ENDING)?;
            self.flush()?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            const LINE_ENDING: &[u8] = b"\n";

            let bufs = &mut [
                std::io::IoSlice::new(s.as_ref().as_bytes()),
                std::io::IoSlice::new(LINE_ENDING),
                std::io::IoSlice::new(&[]), // we need to add a empty one as it may be not written.
            ];

            // As Write trait says it's not guaranteed that write_vectored will write_all data.
            // But we are sure that write_vectored writes everyting or nothing because underthehood it uses a File.
            // But we rely on this fact not explicitely.
            //
            // todo: check amount of written bytes ands write the rest if not everyting was written already.
            let _ = self.write_vectored(bufs)?;
            self.flush()?;

            Ok(())
        }
    }

    fn send_control(&mut self, code: impl TryInto<ControlCode>) -> io::Result<()> {
        let code = code.try_into().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "Failed to parse a control character")
        })?;
        self.stream.write_all(&[code.into()])
    }
}

#[cfg(not(feature = "async"))]
impl<P, S: Stream> Session<P, S> {
    /// Try to read in a non-blocking mode.
    ///
    /// Returns `[std::io::ErrorKind::WouldBlock]`
    /// in case if there's nothing to read.
    pub fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.try_read(buf)
    }

    /// Verifyes if stream is empty or not.
    pub fn is_empty(&mut self) -> io::Result<bool> {
        self.stream.is_empty()
    }
}

#[cfg(not(feature = "async"))]
impl<P: Process, S: Stream> Session<P, S> {
    /// Send `EOF` indicator to a child process.
    ///
    /// Often `eof` char handled as it would be a CTRL-C.
    pub fn send_eof(&mut self) -> io::Result<()> {
        self.stream.write_all(&[self.proc.get_eof_char()?])
    }

    /// Send `INTR` indicator to a child process.
    ///
    /// Often `intr` char handled as it would be a CTRL-D.
    pub fn send_intr(&mut self) -> io::Result<()> {
        self.stream.write_all(&[self.proc.get_intr_char()?])
    }
}

#[cfg(all(feature = "async", not(windows)))]
impl<P: Process + Unpin, S: Stream> Session<P, S> {
    /// Send text to child's `STDIN`.
    ///
    /// To write bytes you can use a [std::io::Write] operations instead.
    pub async fn send(&mut self, s: impl AsRef<str>) -> io::Result<()> {
        self.stream.write_all(s.as_ref().as_bytes()).await
    }

    /// Send a line to child's `STDIN`.
    pub async fn send_line(&mut self, s: impl AsRef<str>) -> io::Result<()> {
        #[cfg(windows)]
        const LINE_ENDING: &[u8] = b"\r\n";
        #[cfg(not(windows))]
        const LINE_ENDING: &[u8] = b"\n";

        let _ = self.write_all(s.as_ref().as_bytes()).await?;
        let _ = self.write_all(LINE_ENDING).await?;
        self.flush().await?;

        Ok(())
    }

    /// Send controll character to a child process.
    ///
    /// You must be carefull passing a char or &str as an argument.
    /// If you pass an unexpected controll you'll get a error.
    /// So it may be better to use [ControlCode].
    ///
    /// ```no_run
    /// use expectrl::{Session, ControlCode};
    /// use std::process::Command;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut process = Session::spawn(Command::new("cat")).unwrap();
    /// process.send_control(ControlCode::EndOfText).await.unwrap(); // sends CTRL^C
    /// process.send_control('C').await.unwrap(); // sends CTRL^C
    /// process.send_control("^C").await.unwrap(); // sends CTRL^C
    /// # });
    /// ```
    pub async fn send_control(&mut self, code: impl TryInto<ControlCode>) -> io::Result<()> {
        let code = code.try_into().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "Failed to parse a control character")
        })?;
        self.stream.write_all(&[code.into()]).await
    }

    /// Send `EOF` indicator to a child process.
    ///
    /// Often `eof` char handled as it would be a CTRL-C.
    pub async fn send_eof(&mut self) -> io::Result<()> {
        self.stream.write_all(&[self.proc.get_eof_char()?]).await
    }

    /// Send `INTR` indicator to a child process.
    ///
    /// Often `intr` char handled as it would be a CTRL-D.
    pub async fn send_intr(&mut self) -> io::Result<()> {
        self.stream.write_all(&[self.proc.get_intr_char()?]).await
    }

    pub(crate) fn get_available(&mut self) -> &[u8] {
        self.stream.get_available()
    }

    pub(crate) fn consume_from_buffer(&mut self, n: usize) {
        self.stream.consume_from_buffer(n);
    }
}

impl<P, S: Stream> Deref for Session<P, S> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.proc
    }
}

impl<P, S: Stream> DerefMut for Session<P, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.proc
    }
}

#[cfg(feature = "async")]
impl<P, S: Stream> Session<P, S> {
    /// Try to read in a non-blocking mode.
    ///
    /// Returns `[std::io::ErrorKind::WouldBlock]`
    /// in case if there's nothing to read.
    pub async fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.try_read(buf).await
    }

    /// Verifyes if stream is empty or not.
    pub async fn is_empty(&mut self) -> io::Result<bool> {
        self.stream.is_empty().await
    }
}

#[cfg(not(feature = "async"))]
impl<P, S: Stream> std::io::Write for Session<P, S> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.stream.write_vectored(bufs)
    }
}

#[cfg(not(feature = "async"))]
impl<P, S: Stream> std::io::Read for Session<P, S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

#[cfg(not(feature = "async"))]
impl<P, S: Stream> std::io::BufRead for Session<P, S> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.stream.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.stream.consume(amt)
    }
}

#[cfg(feature = "async")]
impl<P: Unpin, S: Stream> futures_lite::io::AsyncWrite for Session<P, S> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().stream).poll_close(cx)
    }
}

#[cfg(feature = "async")]
impl<P: Unpin, S: Stream> futures_lite::io::AsyncRead for Session<P, S> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.get_mut().stream).poll_read(cx, buf)
    }
}

#[cfg(feature = "async")]
impl<P: Unpin, S: Stream> futures_lite::io::AsyncBufRead for Session<P, S> {
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        std::pin::Pin::new(&mut self.get_mut().stream).poll_fill_buf(cx)
    }

    fn consume(mut self: std::pin::Pin<&mut Self>, amt: usize) {
        std::pin::Pin::new(&mut self.stream).consume(amt);
    }
}
