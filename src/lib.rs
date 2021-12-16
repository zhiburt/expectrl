//! # A tool for automating terminal applications on Unix and on Windows.
//!
//! Using the library you can:
//!
//! - Spawn process
//! - Control process
//! - Interact with process's IO(input/output).
//!
//! `expectrl` like original `expect` may shine when you're working with interactive applications.
//! If your application is not interactive you may not find the library the best choise.
//!
//! ## Example
//!
//! An example for interacting via ftp.
//!
//! ```no_run,ignore
//! use expectrl::{spawn, Regex, Eof, WaitStatus};
//!
//! let mut p = spawn("ftp speedtest.tele2.net").unwrap();
//! p.expect(Regex("Name \\(.*\\):")).unwrap();
//! p.send_line("anonymous").unwrap();
//! p.expect("Password").unwrap();
//! p.send_line("test").unwrap();
//! p.expect("ftp>").unwrap();
//! p.send_line("cd upload").unwrap();
//! p.expect("successfully changed.\r\nftp>").unwrap();
//! p.send_line("pwd").unwrap();
//! p.expect(Regex("[0-9]+ \"/upload\"")).unwrap();
//! p.send_line("exit").unwrap();
//! p.expect(Eof).unwrap();
//! assert_eq!(p.wait().unwrap(), WaitStatus::Exited(p.pid(), 0));
//! ```
//!
//! *The example inspired by the one in [philippkeller/rexpect].*
//!
//! [For more examples, check the examples directory.](https://github.com/zhiburt/expectrl/tree/main/examples)
//!
//! ## Features
//!
//! - It has an `async` support (To enable them you must turn on an `async` feature).
//! - It supports logging.
//! - It supports interact function.
//! - It has a Windows support.

#[cfg(unix)]
mod check_macros;
mod control_code;
mod error;
mod expect;
pub mod interact;
mod log;
mod process;
pub mod repl;
pub mod session;
mod stream;

pub use control_code::ControlCode;
pub use error::Error;
pub use expect::{Any, Eof, NBytes, Needle, Regex};
pub use session::Found;

pub use process::Stream;

#[cfg(windows)]
pub use conpty::ProcAttr;

#[cfg(unix)]
pub use ptyprocess::{Signal, WaitStatus};

use process::Process as ProcessTrait;
use std::{
    io::{self, Write},
    ops::{Deref, DerefMut},
    process::Command,
};

#[cfg(unix)]
type Process = process::unix::UnixProcess;

#[cfg(windows)]
type Process = process::windows::WindowsProcess;

type ProcessStream = <Process as ProcessTrait>::Stream;

/// Spawn spawnes a new session.
///
/// It accepts a command and possibly arguments just as string.
/// It doesn't parses ENV variables. For complex constrictions use [`Session::spawn`].
///
/// # Example
///
/// ```no_run,ignore
/// use expectrl::{spawn, ControlCode};
/// use std::{thread, time::Duration};
/// use std::io::{Read, Write};
///
/// let mut p = spawn("cat").unwrap();
/// p.send_line("Hello World").unwrap();
///
/// thread::sleep(Duration::from_millis(300)); // give 'cat' some time to set up
/// p.send_control(ControlCode::EndOfText).unwrap(); // abort: SIGINT
///
/// let mut buf = String::new();
/// p.read_to_string(&mut buf).unwrap();
///
/// assert_eq!(buf, "Hello World\r\n");
/// ```
///
/// [`Session::spawn`]: ./struct.Session.html?#spawn
pub fn spawn<S: AsRef<str>>(cmd: S) -> Result<Session<ProcessStream>, Error> {
    let proc = Process::spawn(cmd)?;
    Session::from_process(proc)
}

pub struct Session<S: Stream> {
    inner: session::Session<Process, S>,
}

impl Session<ProcessStream> {
    pub fn spawn(command: Command) -> Result<Self, Error> {
        let mut process = Process::spawn_command(command)?;
        let stream = process.stream()?;
        Self::new(process, stream)
    }

    pub fn from_process(mut process: Process) -> Result<Self, Error> {
        let stream = process.stream()?;
        Self::new(process, stream)
    }
}

impl<S: Stream> Session<S> {
    pub fn new(process: Process, stream: S) -> Result<Self, Error> {
        let session = session::Session::new(process, stream)?;
        Ok(Self { inner: session })
    }

    pub fn with_log<W: Write>(
        mut self,
        logger: W,
    ) -> Result<Session<log::LoggedStream<S, W>>, Error> {
        let (session, old) = self.inner.swap_stream(log::EmptyStream)?;
        let stream = log::LoggedStream::new(old, logger);
        let (session, _) = session.swap_stream(stream)?;

        Ok(Session { inner: session })
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
    pub fn interact(&mut self) -> Result<WaitStatus, Error> {
        crate::interact::InteractOptions::terminal()?.interact(self)
    }

    /// Interact gives control of the child process to the interactive user (the
    /// human at the keyboard).
    #[cfg(windows)]
    pub fn interact(&mut self) -> Result<(), Error> {
        crate::interact::InteractOptions::terminal()?.interact(self)
    }
}

impl<S: Stream> Deref for Session<S> {
    type Target = session::Session<Process, S>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<S: Stream> DerefMut for Session<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(not(feature = "async"))]
impl<S: Stream> io::Write for Session<S> {
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

#[cfg(not(feature = "async"))]
impl<S: Stream> io::Read for Session<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

#[cfg(not(feature = "async"))]
impl<S: Stream> io::BufRead for Session<S> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt)
    }
}

#[cfg(feature = "async")]
impl futures_lite::io::AsyncWrite for Session {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        std::pin::Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_close(cx)
    }
}

#[cfg(feature = "async")]
impl futures_lite::io::AsyncRead for Session {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
        std::pin::Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

#[cfg(feature = "async")]
impl futures_lite::io::AsyncBufRead for Session {
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<&[u8]>> {
        let this = self.get_mut();
        let proc = std::pin::Pin::new(&mut this.stream);
        proc.poll_fill_buf(cx)
    }

    fn consume(mut self: std::pin::Pin<&mut Self>, amt: usize) {
        std::pin::Pin::new(&mut self.stream).consume(amt);
    }
}

// pub struct Builder {
//     command: String,
// }

// pub struct SessionBuilderWithLog<W> {
//     command: String,
//     logger: W,
// }

// impl Builder {
//     pub fn new<S: AsRef<str>>(command: S) -> Self {
//         Self {
//             command: command.as_ref().to_string(),
//         }
//     }

//     pub fn with_log<W: Write>(mut self, logger: W) -> SessionBuilderWithLog<W> {
//         SessionBuilderWithLog {
//             command: self.command,
//             logger,
//         }
//     }

//     pub fn spawn(self) -> Result<Session<ProcessStream>, Error> {
//         let process = Process::spawn(&self.command)?;
//         Session::from_process(process)
//     }
// }

// impl<W: Write> SessionBuilderWithLog<W> {
//     pub fn spawn(self) -> Result<Session<log::LoggedStream<ProcessStream, W>>, Error> {
//         let process = Process::spawn(&self.command)?;
//         let proc_stream = <Process as crate::process::Process>::stream(&mut process)?;
//         let stream = log::LoggedStream::new(proc_stream, self.logger);
//         Session::new(process, stream)
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn test_spawn_no_command() {
        assert!(
            matches!(spawn(""), Err(Error::IO(err)) if err.kind() == io::ErrorKind::InvalidInput && err.to_string() == "a commandline argument is not correct")
        );
    }

    #[test]
    #[ignore = "it's a compile time check"]
    fn session_as_writer() {
        #[cfg(not(feature = "async"))]
        {
            let _: Box<dyn std::io::Write> =
                Box::new(spawn("ls").unwrap()) as Box<dyn std::io::Write>;
            let _: Box<dyn std::io::Read> =
                Box::new(spawn("ls").unwrap()) as Box<dyn std::io::Read>;
            let _: Box<dyn std::io::BufRead> =
                Box::new(spawn("ls").unwrap()) as Box<dyn std::io::BufRead>;

            fn _io_copy<S: Stream>(mut session: Session<S>) {
                std::io::copy(&mut std::io::empty(), &mut session).unwrap();
            }
        }
        #[cfg(feature = "async")]
        {
            let _: Box<dyn futures_lite::AsyncWrite> =
                Box::new(spawn("ls").unwrap()) as Box<dyn futures_lite::AsyncWrite>;
            let _: Box<dyn futures_lite::AsyncRead> =
                Box::new(spawn("ls").unwrap()) as Box<dyn futures_lite::AsyncRead>;
            let _: Box<dyn futures_lite::AsyncBufRead> =
                Box::new(spawn("ls").unwrap()) as Box<dyn futures_lite::AsyncBufRead>;

            async fn _io_copy(mut session: Session) {
                futures_lite::io::copy(futures_lite::io::empty(), &mut session)
                    .await
                    .unwrap();
            }
        }
    }
}
