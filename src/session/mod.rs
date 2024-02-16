//! This module contains a system independent [Session] representation.
//!
//! But it does set a default [Session<P, S>] processes and stream in order to be able to use Session without generics.
//! It also sets a list of other methods which are available for a platform processes.
//!
//! # Example
//!
//! ```no_run,ignore
//! use std::{process::Command, io::prelude::*};
//! use expectrl::Session;
//!
//! let mut p = Session::spawn(Command::new("cat")).unwrap();
//! writeln!(p, "Hello World").unwrap();
//! let mut line = String::new();
//! p.read_line(&mut line).unwrap();
//! ```

#[cfg(feature = "async")]
mod async_session;
mod pty_session;
#[cfg(not(feature = "async"))]
mod sync_session;

pub use pty_session::PtySession;

use std::{io::Write, process::Command};

use crate::{interact::InteractSession, process::Process, stream::log::LogStream, Error};

#[cfg(not(feature = "async"))]
use std::io::Read;

#[cfg(feature = "async")]
use crate::process::IntoAsyncStream;

#[cfg(unix)]
type OsProc = crate::process::unix::UnixProcess;
#[cfg(windows)]
type OsProc = crate::process::windows::WinProcess;

#[cfg(all(unix, not(feature = "async")))]
type OsProcStream = crate::process::unix::PtyStream;
#[cfg(all(unix, feature = "async"))]
type OsProcStream = crate::process::unix::AsyncPtyStream;
#[cfg(all(windows, not(feature = "async")))]
type OsProcStream = crate::process::windows::ProcessStream;
#[cfg(all(windows, feature = "async"))]
type OsProcStream = crate::process::windows::AsyncProcessStream;

/// A type alias for OS process which can run a [`Session`] and a default one.
pub type OsProcess = OsProc;
/// A type alias for OS process stream which is a default one for [`Session`].
pub type OsProcessStream = OsProcStream;

#[cfg(feature = "async")]
pub use async_session::Session;

#[cfg(not(feature = "async"))]
pub use sync_session::Session;

impl Session {
    /// Spawns a session on a platform process.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::process::Command;
    /// use expectrl::Session;
    ///
    /// let p = Session::spawn(Command::new("cat"));
    /// ```
    pub fn spawn(command: Command) -> Result<Self, Error> {
        let mut process = OsProcess::spawn_command(command)?;
        let stream = process.open_stream()?;

        #[cfg(feature = "async")]
        let stream = stream.into_async_stream()?;

        let session = Self::new(process, stream)?;

        Ok(session)
    }

    /// Spawns a session on a platform process.
    /// Using a string commandline.
    pub(crate) fn spawn_cmd(cmd: &str) -> Result<Self, Error> {
        let mut process = OsProcess::spawn(cmd)?;
        let stream = process.open_stream()?;

        #[cfg(feature = "async")]
        let stream = stream.into_async_stream()?;

        let session = Self::new(process, stream)?;

        Ok(session)
    }
}

impl<P, S> Session<P, S> {
    /// Interact gives control of the child process to the interactive user (the
    /// human at the keyboard or a [`Read`]er implementator).
    ///
    /// You can set different callbacks to the session, see [`InteractSession`].
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
    ///
    /// BEWARE that interact finishes after a process stops.
    /// So after the return you may not obtain a correct status of a process.
    ///
    /// In not `async` mode the default version uses a buzy loop.
    ///
    /// - On `linux` you can use a `polling` version using the corresponding feature.
    /// - On `windows` the feature is also present but it spawns a thread for pooling which creates a set of obsticales.
    ///   Specifically if you're planning to call `interact()` multiple times it may not be safe. Because the previous threads may still be running.
    ///
    /// It works via polling in `async` mode on both `unix` and `windows`.
    ///
    /// # Example
    ///
    #[cfg_attr(
        all(unix, not(feature = "async"), not(feature = "polling")),
        doc = "```no_run"
    )]
    #[cfg_attr(
        not(all(unix, not(feature = "async"), not(feature = "polling"))),
        doc = "```ignore"
    )]
    /// use std::io::{stdout, Cursor};
    /// use expectrl::{self, interact::InteractOptions};
    ///
    /// let mut p = expectrl::spawn("cat").unwrap();
    ///
    /// let input = Cursor::new(String::from("Some text right here"));
    ///
    /// p.interact(input, stdout()).spawn(InteractOptions::default()).unwrap();
    /// ```
    ///
    /// [`Read`]: std::io::Read
    pub fn interact<I, O>(&mut self, input: I, output: O) -> InteractSession<&mut Self, I, O> {
        InteractSession::new(self, input, output)
    }
}

/// Set a logger which will write each Read/Write operation into the writter.
///
/// # Example
///
/// ```
/// use expectrl::{spawn, session::log};
///
/// let p = spawn("cat").unwrap();
/// let p = log(p, std::io::stdout());
/// ```
#[cfg(not(feature = "async"))]
pub fn log<W, P, S>(session: Session<P, S>, dst: W) -> Result<Session<P, LogStream<S, W>>, Error>
where
    W: Write,
    S: Read,
{
    session.swap_stream(|s| LogStream::new(s, dst))
}

/// Set a logger which will write each Read/Write operation into the writter.
///
/// # Example
///
/// ```
/// use expectrl::{spawn, session::log};
///
/// let p = spawn("cat").unwrap();
/// let p = log(p, std::io::stdout());
/// ```
#[cfg(feature = "async")]
pub fn log<W, P, S>(session: Session<P, S>, dst: W) -> Result<Session<P, LogStream<S, W>>, Error>
where
    W: Write,
{
    session.swap_stream(|s| LogStream::new(s, dst))
}

