#![allow(clippy::type_complexity)]

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
pub mod async_session;
#[cfg(not(feature = "async"))]
pub mod sync_session;

use std::io::{Read, Write};

use crate::{
    interact::{InteractSession, NoAction, NoFilter},
    process::Process,
    stream::log::LoggedStream,
    Error,
};

#[cfg(feature = "async")]
use crate::process::IntoAsyncStream;

#[cfg(unix)]
pub(crate) type Proc = crate::process::unix::UnixProcess;
#[cfg(all(unix, not(feature = "async")))]
pub(crate) type Stream = crate::process::unix::PtyStream;
#[cfg(all(unix, feature = "async"))]
pub(crate) type Stream = crate::process::unix::AsyncPtyStream;

#[cfg(windows)]
pub(crate) type Proc = crate::process::windows::WinProcess;
#[cfg(all(windows, not(feature = "async")))]
pub(crate) type Stream = crate::process::windows::ProcessStream;
#[cfg(all(windows, feature = "async"))]
pub(crate) type Stream = crate::process::windows::AsyncProcessStream;

/// Session represents a spawned process and its IO stream.
/// It controlls process and communication with it.
///
/// It represents a expect session.
#[cfg(not(feature = "async"))]
pub type Session<P = Proc, S = Stream> = sync_session::Session<P, S>;

/// Session represents a spawned process and its IO stream.
/// It controlls process and communication with it.
///
/// It represents a expect session.
#[cfg(feature = "async")]
pub type Session<P = Proc, S = Stream> = async_session::Session<P, S>;

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
    pub fn spawn(command: <Proc as Process>::Command) -> Result<Self, Error> {
        let mut process = Proc::spawn_command(command)?;
        let stream = process.open_stream()?;

        #[cfg(feature = "async")]
        let stream = stream.into_async_stream()?;

        let session = Self::new(process, stream)?;

        Ok(session)
    }

    /// Spawns a session on a platform process.
    /// Using a string commandline.
    pub(crate) fn spawn_cmd(cmd: &str) -> Result<Self, Error> {
        let mut process = Proc::spawn(cmd)?;
        let stream = process.open_stream()?;

        #[cfg(feature = "async")]
        let stream = stream.into_async_stream()?;

        let session = Self::new(process, stream)?;

        Ok(session)
    }
}

#[cfg(not(feature = "async"))]
impl<P, S: Read> Session<P, S> {
    /// Set a logger which will write each Read/Write operation into the writter.
    ///
    /// # Example
    ///
    /// ```no_run
    /// let p = expectrl::spawn("cat")
    ///     .unwrap()
    ///     .with_log(std::io::stdout())
    ///     .unwrap();
    /// ```
    pub fn with_log<W: Write>(self, logger: W) -> Result<Session<P, LoggedStream<S, W>>, Error> {
        self.swap_stream(|stream| LoggedStream::new(stream, logger))
    }
}

#[cfg(feature = "async")]
impl<P, S> Session<P, S> {
    /// Set a logger which will write each Read/Write operation into the writter.
    ///
    /// # Example
    ///
    /// ```no_run
    /// let p = expectrl::spawn("cat")
    ///     .unwrap()
    ///     .with_log(std::io::stdout())
    ///     .unwrap();
    /// ```
    pub fn with_log<W: Write>(self, logger: W) -> Result<Session<P, LoggedStream<S, W>>, Error> {
        self.swap_stream(|stream| LoggedStream::new(stream, logger))
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
    /// use std::io;
    ///
    /// let mut p = expectrl::spawn("cat").unwrap();
    ///
    /// let input = io::Cursor::new(String::from("Some text right here"));
    ///
    /// p.interact(input, io::stdout()).spawn().unwrap();
    /// ```
    pub fn interact<I, O>(
        &mut self,
        input: I,
        output: O,
    ) -> InteractSession<
        (),
        Self,
        O,
        I,
        NoFilter,
        NoFilter,
        NoAction<Self, O, ()>,
        NoAction<Self, O, ()>,
        NoAction<Self, O, ()>,
    > {
        InteractSession::new(self, output, input, ())
    }
}
