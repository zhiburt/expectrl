use crate::{
    control_code::ControlCode,
    error::Error,
    expect::{Match, Needle},
    stream::Stream,
};
#[cfg(unix)]
use nix::{
    libc::STDIN_FILENO,
    sys::termios,
    unistd::{dup, isatty},
};
#[cfg(unix)]
use ptyprocess::{set_raw, PtyProcess, WaitStatus};
use std::{
    convert::TryInto,
    io::{self, Write},
    ops::{Deref, DerefMut},
    process::Command,
    time::{self, Duration},
};
#[cfg(unix)]
use std::os::unix::prelude::FromRawFd;


#[cfg(feature = "async")]
use futures_lite::AsyncWriteExt;

/// Session represents a process and its streams.
/// It controlls process and communication with it.
#[derive(Debug)]
pub struct Session {
    #[cfg(unix)]
    proc: PtyProcess,
    #[cfg(windows)]
    proc: conpty::Proc,
    stream: Stream,
    expect_timeout: Option<Duration>,
}

impl Session {
    /// Spawn spawns a command
    #[cfg(unix)]
    pub fn spawn(command: Command) -> Result<Self, Error> {
        let ptyproc = PtyProcess::spawn(command)?;
        let stream = Stream::new(ptyproc.get_pty_handle()?);

        Ok(Self {
            proc: ptyproc,
            stream,
            expect_timeout: Some(Duration::from_millis(10000)),
        })
    }

    /// Spawn spawns a command
    #[cfg(windows)]
    pub fn spawn(attr: conpty::ProcAttr) -> Result<Self, Error> {
        let proc = attr.spawn()?;
        let stream = Stream::new(proc.input()?, proc.output()?);

        Ok(Self {
            proc,
            stream,
            expect_timeout: Some(Duration::from_millis(10000)),
        })
    }


    /// Expect waits until a pattern is matched.
    ///
    /// It return error if expect_timeout is reached.
    #[cfg(feature = "async")]
    pub async fn expect<E: Needle>(&mut self, expect: E) -> Result<Found, Error> {
        let start = time::Instant::now();
        let mut eof_reached = false;
        let mut buf = Vec::new();
        loop {
            // We read by byte so there's no need for buffering.
            // If it would read by block's we would be required to create an internal buffer
            // and implement std::io::Read and async_io::AsyncRead to use it.
            // But instead we just reuse it from `ptyprocess` via `Deref`.
            //
            // It's worth to use this approch if there's a performance issue.
            let mut b = [0; 1];
            match self.stream.try_read(&mut b).await {
                Ok(0) => {
                    eof_reached = true;
                }
                Ok(n) => {
                    buf.extend(&b[..n]);
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(Error::IO(err)),
            };

            if let Some(m) = expect.check(&buf, eof_reached)? {
                let buf = buf.drain(..m.end()).collect();
                return Ok(Found::new(buf, m));
            }

            if eof_reached {
                return Err(Error::Eof);
            }

            if let Some(timeout) = self.expect_timeout {
                if start.elapsed() > timeout {
                    return Err(Error::ExpectTimeout);
                }
            }
        }
    }

    /// Expect waits until a pattern is matched.
    ///
    /// It return an error if expect_timeout is reached.
    #[cfg(not(feature = "async"))]
    pub fn expect<E: Needle>(&mut self, expect: E) -> Result<Found, Error> {
        let start = time::Instant::now();
        let mut eof_reached = false;
        let mut buf = Vec::new();
        loop {
            // We read by byte so there's no need for buffering.
            // If it would read by block's we would be required to create an internal buffer
            // and implement std::io::Read and async_io::AsyncRead to use it.
            // But instead we just reuse it from `ptyprocess` via `Deref`.
            //
            // It's worth to use this approch if there's a performance issue.
            let mut b = [0; 1];
            match self.stream.try_read(&mut b) {
                Ok(0) => {
                    eof_reached = true;
                }
                Ok(n) => {
                    buf.extend(&b[..n]);
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(Error::IO(err)),
            };

            if let Some(m) = expect.check(&buf, eof_reached)? {
                let buf = buf.drain(..m.end()).collect();
                return Ok(Found::new(buf, m));
            }

            if eof_reached {
                return Err(Error::Eof);
            }

            if let Some(timeout) = self.expect_timeout {
                if start.elapsed() > timeout {
                    return Err(Error::ExpectTimeout);
                }
            }
        }
    }

    /// Set the pty session's expect timeout.
    pub fn set_expect_timeout(&mut self, expect_timeout: Option<Duration>) {
        self.expect_timeout = expect_timeout;
    }
}

#[cfg(not(feature = "async"))]
impl Session {
    /// Send text to child's `STDIN`.
    ///
    /// To write bytes you can use a [std::io::Write] operations instead.
    pub fn send<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.stream.write_all(s.as_ref().as_bytes())
    }

    /// Send a line to child's `STDIN`.
    pub fn send_line<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        #[cfg(windows)]
        const LINE_ENDING: &[u8] = b"\r\n";
        #[cfg(not(windows))]
        const LINE_ENDING: &[u8] = b"\n";

        let bufs = &mut [
            std::io::IoSlice::new(s.as_ref().as_bytes()),
            std::io::IoSlice::new(LINE_ENDING),
            std::io::IoSlice::new(&[]), // we need to add a empty one as it may be not written.
        ];

        let _ = self.write_vectored(bufs)?;
        self.flush()?;

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
    /// let mut process = Session::spawn(Command::new("cat")).unwrap();
    /// process.send_control(ControlCode::EndOfText); // sends CTRL^C
    /// process.send_control('C'); // sends CTRL^C
    /// process.send_control("^C"); // sends CTRL^C
    /// ```
    pub fn send_control(&mut self, code: impl TryInto<ControlCode>) -> io::Result<()> {
        let code = code.try_into().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "Failed to parse a control character")
        })?;
        self.stream.write_all(&[code.into()])
    }

    /// Send `EOF` indicator to a child process.
    ///
    /// Often `eof` char handled as it would be a CTRL-C.
    #[cfg(unix)]
    pub fn send_eof(&mut self) -> io::Result<()> {
        self.stream.write_all(&[self.proc.get_eof_char()])
    }

    /// Send `INTR` indicator to a child process.
    ///
    /// Often `intr` char handled as it would be a CTRL-D.
    #[cfg(unix)]
    pub fn send_intr(&mut self) -> io::Result<()> {
        self.stream.write_all(&[self.proc.get_intr_char()])
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
    #[cfg(unix)]
    pub fn interact(&mut self) -> io::Result<WaitStatus> {
        // flush buffers
        self.flush()?;

        let origin_pty_echo = self.get_echo().map_err(nix_error_to_io)?;
        self.set_echo(true).map_err(nix_error_to_io)?;

        // verify: possible controlling fd can be stdout and stderr as well?
        // https://stackoverflow.com/questions/35873843/when-setting-terminal-attributes-via-tcsetattrfd-can-fd-be-either-stdout
        let isatty_in = isatty(STDIN_FILENO).map_err(nix_error_to_io)?;

        // tcgetattr issues error if a provided fd is not a tty,
        // so we run set_raw only when it's a tty.
        //
        // todo: simplify.
        if isatty_in {
            let origin_stdin_flags = termios::tcgetattr(STDIN_FILENO).map_err(nix_error_to_io)?;
            set_raw(STDIN_FILENO).map_err(nix_error_to_io)?;

            let result = self._interact();

            termios::tcsetattr(
                STDIN_FILENO,
                termios::SetArg::TCSAFLUSH,
                &origin_stdin_flags,
            )
            .map_err(nix_error_to_io)?;

            self.set_echo(origin_pty_echo).map_err(nix_error_to_io)?;

            result
        } else {
            let result = self._interact();

            self.set_echo(origin_pty_echo).map_err(nix_error_to_io)?;

            result
        }
    }

    #[cfg(unix)]
    fn _interact(&mut self) -> io::Result<WaitStatus> {
        // it's crusial to make a DUP call here.
        // If we don't actual stdin will be closed,
        // And any interaction with it may cause errors.
        //
        // Why we don't use a `std::fs::File::try_clone` with a 0 fd?
        // Because for some reason it actually doesn't make the same things as DUP does,
        // eventhough a research showed that it should.
        // https://github.com/zhiburt/expectrl/issues/7#issuecomment-884787229
        let stdin_copy_fd = dup(STDIN_FILENO).map_err(nix_error_to_io)?;
        let stdin = unsafe { std::fs::File::from_raw_fd(stdin_copy_fd) };
        let mut stdin_stream = Stream::new(stdin);

        let mut buf = [0; 512];
        loop {
            let status = self.status();
            if !matches!(status, Ok(WaitStatus::StillAlive)) {
                return status.map_err(nix_error_to_io);
            }

            // it prints STDIN input as well,
            // by echoing it.
            //
            // the setting must be set before calling the function.
            match self.try_read(&mut buf) {
                Ok(n) => {
                    if n == 0 {
                        // it might be too much to call a `status()` here,
                        // do it just in case.
                        return self.status().map_err(nix_error_to_io);
                    }

                    std::io::stdout().write_all(&buf[..n])?;
                    std::io::stdout().flush()?;
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err),
            }

            match stdin_stream.try_read(&mut buf) {
                Ok(n) => {
                    if n == 0 {
                        // it might be too much to call a `status()` here,
                        // do it just in case.
                        return self.status().map_err(nix_error_to_io);
                    }

                    for i in 0..n {
                        // Ctrl-]
                        if buf[i] == ControlCode::GroupSeparator.into() {
                            // it might be too much to call a `status()` here,
                            // do it just in case.
                            return self.status().map_err(nix_error_to_io);
                        }

                        self.write_all(&buf[i..i + 1])?;
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err),
            }
        }
    }
}

#[cfg(feature = "async")]
impl Session {
    /// Send text to child's `STDIN`.
    ///
    /// To write bytes you can use a [std::io::Write] operations instead.
    pub async fn send<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.stream.write_all(s.as_ref().as_bytes()).await
    }

    /// Send a line to child's `STDIN`.
    pub async fn send_line<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
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
        self.stream.write_all(&[self.proc.get_eof_char()]).await
    }

    /// Send `INTR` indicator to a child process.
    ///
    /// Often `intr` char handled as it would be a CTRL-D.
    pub async fn send_intr(&mut self) -> io::Result<()> {
        self.stream.write_all(&[self.proc.get_intr_char()]).await
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
    pub async fn interact(&mut self) -> io::Result<WaitStatus> {
        // flush buffers
        self.flush().await?;

        let origin_pty_echo = self.get_echo().map_err(nix_error_to_io)?;
        self.set_echo(true).map_err(nix_error_to_io)?;

        // verify: possible controlling fd can be stdout and stderr as well?
        // https://stackoverflow.com/questions/35873843/when-setting-terminal-attributes-via-tcsetattrfd-can-fd-be-either-stdout
        let isatty_in = isatty(STDIN_FILENO).map_err(nix_error_to_io)?;

        // tcgetattr issues error if a provided fd is not a tty,
        // so we run set_raw only when it's a tty.
        //
        // todo: simplify.
        if isatty_in {
            let origin_stdin_flags = termios::tcgetattr(STDIN_FILENO).map_err(nix_error_to_io)?;
            set_raw(STDIN_FILENO).map_err(nix_error_to_io)?;

            let result = self._interact().await;

            termios::tcsetattr(
                STDIN_FILENO,
                termios::SetArg::TCSAFLUSH,
                &origin_stdin_flags,
            )
            .map_err(nix_error_to_io)?;

            self.set_echo(origin_pty_echo).map_err(nix_error_to_io)?;

            result
        } else {
            let result = self._interact().await;

            self.set_echo(origin_pty_echo).map_err(nix_error_to_io)?;

            result
        }
    }

    async fn _interact(&mut self) -> io::Result<WaitStatus> {
        // it's crusial to make a DUP call here.
        // If we don't actual stdin will be closed,
        // And any interaction with it may cause errors.
        //
        // Why we don't use a `std::fs::File::try_clone` with a 0 fd?
        // Because for some reason it actually doesn't make the same things as DUP does,
        // eventhough a research showed that it should.
        // https://github.com/zhiburt/expectrl/issues/7#issuecomment-884787229
        let stdin_copy_fd = dup(0).map_err(nix_error_to_io)?;

        let stdin = unsafe { std::fs::File::from_raw_fd(stdin_copy_fd) };
        let mut stdin_stream = Stream::new(stdin);

        let mut buf = [0; 512];
        loop {
            let status = self.status();
            if !matches!(status, Ok(WaitStatus::StillAlive)) {
                return status.map_err(nix_error_to_io);
            }

            // it prints STDIN input as well,
            // by echoing it.
            //
            // the setting must be set before calling the function.
            match self.try_read(&mut buf).await {
                Ok(n) => {
                    std::io::stdout().write_all(&buf[..n])?;
                    std::io::stdout().flush()?;
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err),
            }

            match stdin_stream.try_read(&mut buf).await {
                Ok(n) => {
                    for i in 0..n {
                        // Ctrl-]
                        if buf[i] == ControlCode::GroupSeparator.into() {
                            // it might be too much to call a `status()` here,
                            // do it just in case.
                            return self.status().map_err(nix_error_to_io);
                        }

                        self.write_all(&buf[i..i + 1]).await?;
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err),
            }
        }
    }
}

#[cfg(unix)]
impl Deref for Session {
    type Target = PtyProcess;

    fn deref(&self) -> &Self::Target {
        &self.proc
    }
}

#[cfg(unix)]
impl DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.proc
    }
}

#[cfg(windows)]
impl Deref for Session {
    type Target = conpty::Proc;

    fn deref(&self) -> &Self::Target {
        &self.proc
    }
}

#[cfg(windows)]
impl DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.proc
    }
}

/// Found is a represention of a matched pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    buf: Vec<u8>,
    m: Match,
}

impl Found {
    /// New returns an instance of Found.
    pub fn new(buf: Vec<u8>, m: Match) -> Self {
        Self { buf, m }
    }

    /// Found_match returns a matched bytes.
    pub fn found_match(&self) -> &[u8] {
        &self.buf[self.m.start()..self.m.end()]
    }

    /// Before_match returns a bytes before match.
    pub fn before_match(&self) -> &[u8] {
        &self.buf[..self.m.start()]
    }
}

#[cfg(feature = "async")]
impl Session {
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
impl Session {
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
impl std::io::Write for Session {
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
impl std::io::Read for Session {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

#[cfg(not(feature = "async"))]
impl std::io::BufRead for Session {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.stream.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.stream.consume(amt)
    }
}

#[cfg(feature = "async")]
impl futures_lite::io::AsyncWrite for Session {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_close(cx)
    }
}

#[cfg(feature = "async")]
impl futures_lite::io::AsyncRead for Session {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        futures_lite::io::AsyncRead::poll_read(std::pin::Pin::new(&mut self.stream), cx, buf)
    }
}

#[cfg(feature = "async")]
impl futures_lite::io::AsyncBufRead for Session {
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        let this = self.get_mut();
        let proc = std::pin::Pin::new(&mut this.stream);
        proc.poll_fill_buf(cx)
    }

    fn consume(mut self: std::pin::Pin<&mut Self>, amt: usize) {
        std::pin::Pin::new(&mut self.stream).consume(amt);
    }
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
