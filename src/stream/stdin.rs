//! The module contains a nonblocking version of [std::io::Stdin].  

use std::io;

#[cfg(not(feature = "async"))]
use std::io::Read;

#[cfg(feature = "async")]
use std::{
    pin::Pin,
    task::{Context, Poll},
};

#[cfg(feature = "async")]
use futures_lite::AsyncRead;

use crate::Error;

/// A non blocking version of STDIN.
///
/// It's not recomended to be used directly.
/// But we expose it because it cab be used with [`Session::interact`].
///
/// [`Session::interact`]: crate::session::Session::interact
#[derive(Debug)]
pub struct Stdin {
    inner: inner::StdinInner,
}

impl Stdin {
    /// Creates a new instance of Stdin.
    ///
    /// It may change terminal's STDIN state therefore, after
    /// it's used you must call [Stdin::close].
    pub fn open() -> Result<Self, Error> {
        #[cfg(not(feature = "async"))]
        {
            let mut stdin = inner::StdinInner::new().map(|inner| Self { inner })?;
            stdin.blocking(true)?;
            Ok(stdin)
        }

        #[cfg(feature = "async")]
        {
            let stdin = inner::StdinInner::new().map(|inner| Self { inner })?;
            Ok(stdin)
        }
    }

    /// Close frees a resources which were used.
    ///
    /// It must be called [Stdin] was used.
    /// Otherwise the STDIN might be returned to original state.
    pub fn close(mut self) -> Result<(), Error> {
        #[cfg(not(feature = "async"))]
        self.blocking(false)?;
        self.inner.close()?;
        Ok(())
    }

    #[cfg(not(feature = "async"))]
    pub(crate) fn blocking(&mut self, on: bool) -> Result<(), Error> {
        self.inner.blocking(on)
    }
}

#[cfg(not(feature = "async"))]
impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

#[cfg(feature = "async")]
impl AsyncRead for Stdin {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        AsyncRead::poll_read(Pin::new(&mut self.inner), cx, buf)
    }
}

#[cfg(unix)]
impl std::os::unix::prelude::AsRawFd for Stdin {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.inner.as_raw_fd()
    }
}

#[cfg(unix)]
impl std::os::unix::prelude::AsRawFd for &mut Stdin {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.inner.as_raw_fd()
    }
}

#[cfg(all(unix, feature = "polling"))]
impl polling::Source for Stdin {
    fn raw(&self) -> std::os::unix::prelude::RawFd {
        std::os::unix::io::AsRawFd::as_raw_fd(self)
    }
}

#[cfg(unix)]
mod inner {
    use super::*;

    use std::os::unix::prelude::AsRawFd;

    use nix::{
        libc::STDIN_FILENO,
        sys::termios::{self, Termios},
        unistd::isatty,
    };
    use ptyprocess::set_raw;

    #[derive(Debug)]
    pub(super) struct StdinInner {
        orig_flags: Option<Termios>,
        #[cfg(feature = "async")]
        stdin: async_io::Async<io::Stdin>,
        #[cfg(not(feature = "async"))]
        stdin: io::Stdin,
    }

    impl StdinInner {
        pub(super) fn new() -> Result<Self, Error> {
            let stdin = io::stdin();
            #[cfg(feature = "async")]
            let stdin = async_io::Async::new(stdin)?;

            #[cfg(target_os = "macos")]
            let orig_flags = None;

            #[cfg(not(target_os = "macos"))]
            let orig_flags = Self::prepare()?;

            Ok(Self { stdin, orig_flags })
        }

        pub(super) fn prepare() -> Result<Option<Termios>, Error> {
            // flush buffers
            // self.stdin.flush()?;

            let mut o_pty_flags = None;

            // verify: possible controlling fd can be stdout and stderr as well?
            // https://stackoverflow.com/questions/35873843/when-setting-terminal-attributes-via-tcsetattrfd-can-fd-be-either-stdout
            let isatty_terminal = isatty(STDIN_FILENO)
                .map_err(|e| Error::unknown("failed to call isatty", e.to_string()))?;
            if isatty_terminal {
                // tcgetattr issues error if a provided fd is not a tty,
                // but we can work with such input as it may be redirected.
                o_pty_flags = termios::tcgetattr(STDIN_FILENO)
                    .map(Some)
                    .map_err(|e| Error::unknown("failed to call tcgetattr", e.to_string()))?;

                set_raw(STDIN_FILENO)
                    .map_err(|e| Error::unknown("failed to set a raw tty", e.to_string()))?;
            }

            Ok(o_pty_flags)
        }

        pub(super) fn close(&mut self) -> Result<(), Error> {
            if let Some(origin_stdin_flags) = &self.orig_flags {
                termios::tcsetattr(STDIN_FILENO, termios::SetArg::TCSAFLUSH, origin_stdin_flags)
                    .map_err(|e| Error::unknown("failed to call tcsetattr", e.to_string()))?;
            }

            Ok(())
        }

        #[cfg(not(feature = "async"))]
        pub(crate) fn blocking(&mut self, on: bool) -> Result<(), Error> {
            crate::process::unix::make_non_blocking(self.as_raw_fd(), on).map_err(Error::IO)
        }
    }

    impl AsRawFd for StdinInner {
        fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
            self.stdin.as_raw_fd()
        }
    }

    #[cfg(not(feature = "async"))]
    impl Read for StdinInner {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.stdin.read(buf)
        }
    }

    #[cfg(feature = "async")]
    impl AsyncRead for StdinInner {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            AsyncRead::poll_read(Pin::new(&mut self.stdin), cx, buf)
        }
    }
}

#[cfg(windows)]
mod inner {
    use super::*;

    use conpty::console::Console;

    pub(super) struct StdinInner {
        terminal: Console,
        #[cfg(not(feature = "async"))]
        is_blocking: bool,
        #[cfg(not(feature = "async"))]
        stdin: io::Stdin,
        #[cfg(feature = "async")]
        stdin: blocking::Unblock<io::Stdin>,
    }

    impl std::fmt::Debug for StdinInner {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            #[cfg(not(feature = "async"))]
            {
                f.debug_struct("StdinInner")
                    .field("terminal", &self.terminal)
                    .field("is_blocking", &self.is_blocking)
                    .field("stdin", &self.stdin)
                    .field("stdin", &self.stdin)
                    .finish()
            }
            #[cfg(feature = "async")]
            {
                f.debug_struct("StdinInner")
                    .field("terminal", &self.terminal)
                    .field("stdin", &self.stdin)
                    .field("stdin", &self.stdin)
                    .finish()
            }
        }
    }

    impl StdinInner {
        /// Creates a new instance of Stdin.
        ///
        /// It changes terminal's STDIN state therefore, after
        /// it's used please call [Stdin::close].
        pub(super) fn new() -> Result<Self, Error> {
            let console = conpty::console::Console::current().map_err(to_io_error)?;
            console.set_raw().map_err(to_io_error)?;

            let stdin = io::stdin();

            #[cfg(feature = "async")]
            let stdin = blocking::Unblock::new(stdin);

            Ok(Self {
                terminal: console,
                #[cfg(not(feature = "async"))]
                is_blocking: false,
                stdin,
            })
        }

        pub(super) fn close(&mut self) -> Result<(), Error> {
            self.terminal.reset().map_err(to_io_error)?;
            Ok(())
        }

        #[cfg(not(feature = "async"))]
        pub(crate) fn blocking(&mut self, on: bool) -> Result<(), Error> {
            self.is_blocking = on;
            Ok(())
        }
    }

    #[cfg(not(feature = "async"))]
    impl Read for StdinInner {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            // fixme: I am not sure why reading works on is_stdin_empty() == true
            // maybe rename of the method necessary
            if self.is_blocking && !self.terminal.is_stdin_empty().map_err(to_io_error)? {
                return Err(io::Error::new(io::ErrorKind::WouldBlock, ""));
            }

            self.stdin.read(buf)
        }
    }

    #[cfg(feature = "async")]
    impl AsyncRead for StdinInner {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            AsyncRead::poll_read(Pin::new(&mut self.stdin), cx, buf)
        }
    }

    fn to_io_error(err: impl std::error::Error) -> io::Error {
        io::Error::new(io::ErrorKind::Other, err.to_string())
    }

    #[cfg(all(feature = "polling", not(feature = "async")))]
    impl Clone for StdinInner {
        fn clone(&self) -> Self {
            Self {
                terminal: self.terminal.clone(),
                is_blocking: self.is_blocking.clone(),
                stdin: std::io::stdin(),
            }
        }
    }
}

#[cfg(all(windows, feature = "polling", not(feature = "async")))]
impl Clone for Stdin {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
