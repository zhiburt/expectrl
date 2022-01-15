//! Module contains a Session structure.

use std::{
    convert::TryInto,
    io::{self, BufRead, Read, Write},
    ops::{Deref, DerefMut},
    time::{self, Duration},
};

use crate::{
    control_code::ControlCode,
    error::Error,
    needle::Needle,
    stream::log::LoggedStream,
    Found,
};

use super::stream::{TryStream, NonBlocking};

#[cfg(unix)]
pub type Session = PtySession<ptyprocess::PtyProcess, LoggedStream<'static, crate::stream::unix::PtyStream>>;

#[cfg(windows)]
pub type Session = PtySession<conpty::Process, LoggedStream<'static, crate::stream::windows::ProcessStream>>;

impl Session {
    #[cfg(unix)]
    pub fn spawn(command: std::process::Command) -> Result<Self, Error> {
        let process = ptyprocess::PtyProcess::spawn(command)?;
        let stream = crate::stream::unix::PtyStream::new(process.get_pty_stream()?);
        let logged_stream = LoggedStream::new(stream, io::sink());
        let session = Self::new(process, logged_stream)?;

        Ok(session)
    }

    #[cfg(windows)]
    pub fn spawn(attr: conpty::ProcAttr) -> Result<Self, Error> {
        let process = attr.spawn()?;
        let stream = crate::stream::windows::ProcessStream::new(process.output()?, process.input()?);
        let logged_stream = LoggedStream::new(stream, io::sink());
        let session = Self::new(process, logged_stream)?;

        Ok(session)
    }

    /// Set logger.
    pub fn set_log<W: io::Write + Send + 'static>(&mut self, logger: W) -> io::Result<()> {
        self.stream.get_mut().set_logger(logger);
        Ok(())
    }
}

impl Session {
    /// Interact gives control of the child process to the interactive user (the
    /// human at the keyboard).
    ///
    /// Returns a status of a process ater interactions.
    /// Why it's crusial to return a status is after check of is_alive the actuall
    /// status might be gone.
    ///
    /// Keystrokes are sent to the child process, and
    /// the `stdout` and `stderr` output of the child process is printed.
    ///
    /// When the user types the `escape_character` this method will return control to a running process.
    /// The escape_character will not be transmitted.
    /// The default for escape_character is entered as `Ctrl-]`, the very same as BSD telnet.
    ///
    /// This simply echos the child `stdout` and `stderr` to the real `stdout` and
    /// it echos the real `stdin` to the child `stdin`.

    #[cfg(unix)]
    #[cfg(not(feature = "async"))]
    pub fn interact(&mut self) -> Result<crate::WaitStatus, Error> {
        crate::interact::InteractOptions::terminal()?.interact(self)
    }

    /// Interact gives control of the child process to the interactive user (the
    /// human at the keyboard).
    ///
    /// Returns a status of a process ater interactions.
    /// Why it's crusial to return a status is after check of is_alive the actuall
    /// status might be gone.
    ///
    /// Keystrokes are sent to the child process, and
    /// the `stdout` and `stderr` output of the child process is printed.
    ///
    /// When the user types the `escape_character` this method will return control to a running process.
    /// The escape_character will not be transmitted.
    /// The default for escape_character is entered as `Ctrl-]`, the very same as BSD telnet.
    ///
    /// This simply echos the child `stdout` and `stderr` to the real `stdout` and
    /// it echos the real `stdin` to the child `stdin`.
    // #[cfg(unix)]
    // #[cfg(feature = "async")]
    // pub async fn interact(&mut self) -> Result<WaitStatus, Error> {
    //     crate::interact::InteractOptions::terminal()?
    //         .interact(self)
    //         .await
    // }

    /// Interact gives control of the child process to the interactive user (the
    /// human at the keyboard).
    #[cfg(windows)]
    pub fn interact(&mut self) -> Result<(), Error> {
        crate::interact::InteractOptions::terminal()?.interact(self)
    }
}

/// Session represents a spawned process and its streams.
/// It controlls process and communication with it.
#[derive(Debug)]
pub struct PtySession<P, S> {
    proc: P,
    stream: TryStream<S>,
    expect_timeout: Option<Duration>,
}

impl<P, S: Read> PtySession<P, S> {
    //
    pub fn new(process: P, stream: S) -> io::Result<Self> {
        let stream = TryStream::new(stream)?;
        Ok(Self {
            proc: process,
            stream,
            expect_timeout: Some(Duration::from_millis(10000)),
        })
    }
}

impl<P, S> PtySession<P, S> {
    /// Set the pty session's expect timeout.
    pub fn set_expect_timeout(&mut self, expect_timeout: Option<Duration>) {
        self.expect_timeout = expect_timeout;
    }
}

impl<P, S: Read + NonBlocking> PtySession<P, S> {
    pub fn expect<E: Needle>(&mut self, expect: E) -> Result<Found, Error> {
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

    pub fn check<E: Needle>(&mut self, needle: E) -> Result<Found, Error> {
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

    pub fn is_matched<E: Needle>(&mut self, needle: E) -> Result<bool, Error> {
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
}

impl<P, S: Write> PtySession<P, S> {
    pub fn send(&mut self, s: impl AsRef<str>) -> io::Result<()> {
        self.stream.write_all(s.as_ref().as_bytes())
    }

    pub fn send_line(&mut self, s: impl AsRef<str>) -> io::Result<()> {
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

    pub fn send_control(&mut self, code: impl TryInto<ControlCode>) -> io::Result<()> {
        let code = code.try_into().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "Failed to parse a control character")
        })?;
        self.stream.write_all(&[code.into()])
    }
}

impl<P, S: Read + NonBlocking> PtySession<P, S> {
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

impl<P, S: Write> Write for PtySession<P, S> {
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

impl<P, S: Read> Read for PtySession<P, S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

impl<P, S: Read> BufRead for PtySession<P, S> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.stream.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.stream.consume(amt)
    }
}

impl<P, S> Deref for PtySession<P, S> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.proc
    }
}

impl<P, S> DerefMut for PtySession<P, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.proc
    }
}