//! The module contains a nonblocking version of [std::io::Stdin].  

use crate::Error;
use std::io::{self, Read};

#[cfg(unix)]
use nix::{
    libc::STDIN_FILENO,
    sys::termios::{self, Termios},
    unistd::isatty,
};
#[cfg(unix)]
use ptyprocess::{set_raw, PtyProcess};
#[cfg(unix)]
use std::os::unix::prelude::AsRawFd;

#[cfg(windows)]
use crate::process::windows::WinProcess;
#[cfg(windows)]
use conpty::console::Console;

/// A non blocking version of STDIN.
///
/// It's not recomended to be used directly.
/// But we expose it because its used in [crate::interact::InteractOptions::interact_in_terminal].
#[cfg(unix)]
pub struct Stdin {
    stdin: io::Stdin,
    orig_flags: Option<Termios>,
    orig_echo: bool,
}

#[cfg(unix)]
impl Stdin {
    /// Creates a new instance of Stdin.
    ///
    /// It changes terminal's STDIN state therefore, after
    /// it's used please call [Stdin::close].
    pub fn new(pty: &mut PtyProcess) -> Result<Self, Error> {
        let stdin = io::stdin();
        let mut stdin = Self {
            stdin,
            orig_flags: None,
            orig_echo: false,
        };

        stdin.prepare(pty)?;

        Ok(stdin)
    }

    fn prepare(&mut self, pty: &mut PtyProcess) -> Result<(), Error> {
        // flush buffers
        // self.stdin.flush()?;

        let mut o_pty_flags = None;
        let o_pty_echo = pty
            .get_echo()
            .map_err(|e| Error::unknown("failed to get echo", e))?;

        // verify: possible controlling fd can be stdout and stderr as well?
        // https://stackoverflow.com/questions/35873843/when-setting-terminal-attributes-via-tcsetattrfd-can-fd-be-either-stdout
        let isatty_terminal =
            isatty(STDIN_FILENO).map_err(|e| Error::unknown("failed to call isatty", e))?;
        if isatty_terminal {
            // tcgetattr issues error if a provided fd is not a tty,
            // but we can work with such input as it may be redirected.
            o_pty_flags = termios::tcgetattr(STDIN_FILENO)
                .map(Some)
                .map_err(|e| Error::unknown("failed to call tcgetattr", e))?;

            set_raw(STDIN_FILENO).map_err(|e| Error::unknown("failed to set a raw tty", e))?;
        }

        pty.set_echo(true, None)
            .map_err(|e| Error::unknown("failed to set echo", e))?;

        self.orig_echo = o_pty_echo;
        self.orig_flags = o_pty_flags;

        Ok(())
    }

    /// Close frees a resources which were used.
    ///
    /// It must be called [Stdin] was used.
    /// Otherwise the STDIN might be returned to original state.
    pub fn close(self, pty: &mut PtyProcess) -> Result<(), Error> {
        if let Some(origin_stdin_flags) = self.orig_flags {
            termios::tcsetattr(
                STDIN_FILENO,
                termios::SetArg::TCSAFLUSH,
                &origin_stdin_flags,
            )
            .map_err(|e| Error::unknown("failed to call tcsetattr", e))?;
        }

        pty.set_echo(self.orig_echo, None)
            .map_err(|e| Error::unknown("failed to set echo", e))?;
        Ok(())
    }
}

#[cfg(unix)]
impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        crate::process::unix::_make_non_blocking(self.stdin.as_raw_fd(), true)?;

        let result = match self.stdin.read(buf) {
            Ok(n) => Ok(n),
            Err(err) => Err(err),
        };

        crate::process::unix::_make_non_blocking(self.stdin.as_raw_fd(), false)?;

        result
    }
}

#[cfg(unix)]
#[cfg(feature = "async")]
impl futures_lite::AsyncRead for Stdin {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
        std::task::Poll::Ready(self.read(buf))
    }
}

/// A non blocking version of STDIN.
///
/// It's not recomended to be used directly.
/// But we expose it because its used in [crate::interact::InteractOptions::interact_in_terminal].
#[cfg(windows)]
pub struct Stdin {
    current_terminal: Console,
}

#[cfg(windows)]
impl Stdin {
    /// Creates a new instance of Stdin.
    ///
    /// It changes terminal's STDIN state therefore, after
    /// it's used please call [Stdin::close].
    pub fn new(_session: &mut WinProcess) -> Result<Self, Error> {
        let console = conpty::console::Console::current().map_err(to_io_error)?;
        let mut stdin = Self {
            current_terminal: console,
        };
        stdin.prepare()?;
        Ok(stdin)
    }

    fn prepare(&mut self) -> Result<(), Error> {
        self.current_terminal.set_raw().map_err(to_io_error)?;
        Ok(())
    }

    /// Close frees a resources which were used.
    ///
    /// It must be called [Stdin] was used.
    /// Otherwise the STDIN might be returned to original state.
    pub fn close(&mut self, _session: &mut WinProcess) -> Result<(), Error> {
        self.current_terminal.reset().map_err(to_io_error)?;
        Ok(())
    }
}

#[cfg(windows)]
impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // we can't easily read in non-blocking manner,
        // but we can check when there's something to read,
        // which seems to be enough to not block.
        //
        // fixme: I am not sure why reading works on is_stdin_empty() == true
        if self
            .current_terminal
            .is_stdin_empty()
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
        {
            io::stdin().read(buf)
        } else {
            Err(io::Error::new(io::ErrorKind::WouldBlock, ""))
        }
    }
}

#[cfg(windows)]
#[cfg(feature = "async")]
impl futures_lite::AsyncRead for Stdin {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
        std::task::Poll::Ready(self.read(buf))
    }
}

#[cfg(windows)]
fn to_io_error(err: impl std::error::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err.to_string())
}
