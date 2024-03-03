//! Module contains a Session structure.

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    time::{self, Duration},
};

use crate::{
    error::Error,
    expect::Expect,
    needle::Needle,
    process::{Healthcheck, NonBlocking, Termios},
    Captures,
};

/// Session represents a spawned process and its streams.
/// It controlls process and communication with it.
#[derive(Debug)]
pub struct Session<P, S> {
    proc: P,
    stream: TryStream<S>,
    expect_timeout: Option<Duration>,
    expect_lazy: bool,
}

impl<P, S> Session<P, S>
where
    S: Read,
{
    /// Creates a new session.
    pub fn new(process: P, stream: S) -> io::Result<Self> {
        let stream = TryStream::new(stream)?;

        Ok(Self {
            proc: process,
            stream,
            expect_timeout: Some(Duration::from_millis(10000)),
            expect_lazy: false,
        })
    }

    pub(crate) fn swap_stream<F, R>(mut self, new: F) -> Result<Session<P, R>, Error>
    where
        F: FnOnce(S) -> R,
        R: Read,
    {
        self.stream.flush_in_buffer();
        let buf = self.stream.get_available().to_owned();

        let stream = self.stream.into_inner();
        let stream = new(stream);

        let mut session = Session::new(self.proc, stream)?;
        session.stream.keep_in_buffer(&buf);

        Ok(session)
    }
}

impl<P, S> Session<P, S> {
    /// Set the pty session's expect timeout.
    pub fn set_expect_timeout(&mut self, expect_timeout: Option<Duration>) {
        self.expect_timeout = expect_timeout;
    }

    /// Set a expect algorithm to be either gready or lazy.
    ///
    /// Default algorithm is gready.
    ///
    /// See [Session::expect].
    pub fn set_expect_lazy(&mut self, lazy: bool) {
        self.expect_lazy = lazy;
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
        &self.proc
    }

    /// Get a mut reference to a process running program.
    pub fn get_process_mut(&mut self) -> &mut P {
        &mut self.proc
    }
}

impl<P, S> Expect for Session<P, S>
where
    S: Write + Read + NonBlocking,
{
    fn expect<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle,
    {
        match self.expect_lazy {
            true => self.expect_lazy(needle),
            false => self.expect_gready(needle),
        }
    }

    fn check<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle,
    {
        let eof = self.stream.read_available()?;
        let buf = self.stream.get_available();

        let found = needle.check(buf, eof)?;
        if !found.is_empty() {
            let end_index = Captures::right_most_index(&found);
            let involved_bytes = buf[..end_index].to_vec();
            self.stream.consume_available(end_index);
            return Ok(Captures::new(involved_bytes, found));
        }

        if eof {
            return Err(Error::Eof);
        }

        Ok(Captures::new(Vec::new(), Vec::new()))
    }

    fn is_matched<N>(&mut self, needle: N) -> Result<bool, Error>
    where
        N: Needle,
    {
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

    fn send<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>,
    {
        self.stream.write_all(buf.as_ref())?;

        Ok(())
    }

    fn send_line<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>,
    {
        #[cfg(windows)]
        const LINE_ENDING: &[u8] = b"\r\n";
        #[cfg(not(windows))]
        const LINE_ENDING: &[u8] = b"\n";

        self.stream.write_all(buf.as_ref())?;
        self.write_all(LINE_ENDING)?;

        Ok(())
    }
}

impl<P, S> Session<P, S>
where
    S: Read + NonBlocking,
{
    /// Expect which fills as much as possible to the buffer.
    ///
    /// See [Session::expect].
    fn expect_gready<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle,
    {
        let start = time::Instant::now();
        loop {
            let eof = self.stream.read_available()?;
            let data = self.stream.get_available();

            let found = needle.check(data, eof)?;
            if !found.is_empty() {
                let end_index = Captures::right_most_index(&found);
                let involved_bytes = data[..end_index].to_vec();
                self.stream.consume_available(end_index);

                return Ok(Captures::new(involved_bytes, found));
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

    /// Expect which reads byte by byte.
    ///
    /// See [Session::expect].
    fn expect_lazy<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle,
    {
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

            let found = needle.check(data, eof)?;
            if !found.is_empty() {
                let end_index = Captures::right_most_index(&found);
                let involved_bytes = data[..end_index].to_vec();
                self.stream.consume_available(end_index);
                return Ok(Captures::new(involved_bytes, found));
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
}

impl<P, S> Session<P, S>
where
    S: Read + NonBlocking,
{
    /// Try to read in a non-blocking mode.
    ///
    /// Returns [`std::io::ErrorKind::WouldBlock`]
    /// in case there's nothing to read.
    pub fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.try_read(buf)
    }

    /// Verifyes if stream is empty or not.
    pub fn is_empty(&mut self) -> io::Result<bool> {
        self.stream.is_empty()
    }
}

impl<P, S> Write for Session<P, S>
where
    S: Write,
{
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

impl<P, S> Read for Session<P, S>
where
    S: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

impl<P, S> BufRead for Session<P, S>
where
    S: Read,
{
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.stream.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.stream.consume(amt)
    }
}

impl<P, S> Healthcheck for Session<P, S>
where
    P: Healthcheck,
{
    type Status = P::Status;

    fn get_status(&self) -> io::Result<Self::Status> {
        self.get_process().get_status()
    }

    fn is_alive(&self) -> io::Result<bool> {
        self.get_process().is_alive()
    }
}

impl<P, S> Termios for Session<P, S>
where
    P: Termios,
{
    fn is_echo(&self) -> io::Result<bool> {
        self.get_process().is_echo()
    }

    fn set_echo(&mut self, on: bool) -> io::Result<bool> {
        self.get_process_mut().set_echo(on)
    }
}

impl<P, S> NonBlocking for Session<P, S>
where
    S: NonBlocking,
{
    fn set_blocking(&mut self, on: bool) -> io::Result<()> {
        S::set_blocking(self.get_stream_mut(), on)
    }
}

#[cfg(unix)]
impl<P, S> std::os::fd::AsRawFd for Session<P, S>
where
    S: std::os::fd::AsRawFd,
{
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.get_stream().as_raw_fd()
    }
}

#[cfg(unix)]
impl<P, S> std::os::fd::AsRawFd for &Session<P, S>
where
    S: std::os::fd::AsRawFd,
{
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.get_stream().as_raw_fd()
    }
}

#[cfg(unix)]
impl<P, S> std::os::fd::AsRawFd for &mut Session<P, S>
where
    S: std::os::fd::AsRawFd,
{
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.get_stream().as_raw_fd()
    }
}

#[derive(Debug)]
struct TryStream<S> {
    stream: ControlledReader<S>,
}

impl<S> TryStream<S> {
    fn into_inner(self) -> S {
        self.stream.inner.into_inner().inner
    }

    fn as_ref(&self) -> &S {
        &self.stream.inner.get_ref().inner
    }

    fn as_mut(&mut self) -> &mut S {
        &mut self.stream.inner.get_mut().inner
    }
}

impl<S> TryStream<S>
where
    S: Read,
{
    /// The function returns a new Stream from a file.
    fn new(stream: S) -> io::Result<Self> {
        Ok(Self {
            stream: ControlledReader::new(stream),
        })
    }

    fn flush_in_buffer(&mut self) {
        self.stream.flush_in_buffer();
    }
}

impl<S> TryStream<S> {
    fn keep_in_buffer(&mut self, v: &[u8]) {
        self.stream.keep_in_buffer(v);
    }

    fn get_available(&mut self) -> &[u8] {
        self.stream.get_available()
    }

    fn consume_available(&mut self, n: usize) {
        self.stream.consume_available(n)
    }
}

impl<R> TryStream<R>
where
    R: Read + NonBlocking,
{
    /// Try to read in a non-blocking mode.
    ///
    /// It raises io::ErrorKind::WouldBlock if there's nothing to read.
    fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.get_mut().set_blocking(false)?;

        let result = self.stream.inner.read(buf);

        // As file is DUPed changes in one descriptor affects all ones
        // so we need to make blocking file after we finished.
        self.stream.get_mut().set_blocking(true)?;

        result
    }

    #[allow(clippy::wrong_self_convention)]
    fn is_empty(&mut self) -> io::Result<bool> {
        match self.try_read(&mut []) {
            Ok(0) => Ok(true),
            Ok(_) => Ok(false),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(true),
            Err(err) => Err(err),
        }
    }

    fn read_available(&mut self) -> std::io::Result<bool> {
        self.stream.flush_in_buffer();

        let mut buf = [0; 248];
        loop {
            match self.try_read_inner(&mut buf) {
                Ok(0) => break Ok(true),
                Ok(n) => {
                    self.stream.keep_in_buffer(&buf[..n]);
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break Ok(false),
                Err(err) => break Err(err),
            }
        }
    }

    fn read_available_once(&mut self, buf: &mut [u8]) -> std::io::Result<Option<usize>> {
        self.stream.flush_in_buffer();

        match self.try_read_inner(buf) {
            Ok(0) => Ok(Some(0)),
            Ok(n) => {
                self.stream.keep_in_buffer(&buf[..n]);
                Ok(Some(n))
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(err),
        }
    }

    // non-buffered && non-blocking read
    fn try_read_inner(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.get_mut().set_blocking(false)?;

        let result = self.stream.get_mut().read(buf);

        // As file is DUPed changes in one descriptor affects all ones
        // so we need to make blocking file after we finished.
        self.stream.get_mut().set_blocking(true)?;

        result
    }
}

impl<S> Write for TryStream<S>
where
    S: Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.inner.get_mut().inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.inner.get_mut().inner.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.stream.inner.get_mut().inner.write_vectored(bufs)
    }
}

impl<R> Read for TryStream<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.inner.read(buf)
    }
}

impl<R> BufRead for TryStream<R>
where
    R: Read,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.stream.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.stream.inner.consume(amt)
    }
}

#[derive(Debug)]
struct ControlledReader<R> {
    inner: BufReader<BufferedReader<R>>,
}

impl<R> ControlledReader<R>
where
    R: Read,
{
    fn new(reader: R) -> Self {
        Self {
            inner: BufReader::new(BufferedReader::new(reader)),
        }
    }

    fn flush_in_buffer(&mut self) {
        // Because we have 2 buffered streams there might appear inconsistancy
        // in read operations and the data which was via `keep_in_buffer` function.
        //
        // To eliminate it we move BufReader buffer to our buffer.
        let b = self.inner.buffer().to_vec();
        self.inner.consume(b.len());
        self.keep_in_buffer(&b);
    }
}

impl<R> ControlledReader<R> {
    fn keep_in_buffer(&mut self, v: &[u8]) {
        self.inner.get_mut().buffer.extend(v);
    }

    fn get_mut(&mut self) -> &mut R {
        &mut self.inner.get_mut().inner
    }

    fn get_available(&mut self) -> &[u8] {
        &self.inner.get_ref().buffer
    }

    fn consume_available(&mut self, n: usize) {
        let _ = self.inner.get_mut().buffer.drain(..n);
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

impl<R> Read for BufferedReader<R>
where
    R: Read,
{
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        if self.buffer.is_empty() {
            self.inner.read(buf)
        } else {
            let n = buf.write(&self.buffer)?;
            let _ = self.buffer.drain(..n);
            Ok(n)
        }
    }
}
