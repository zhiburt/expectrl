//! This module contains a platform independent abstraction over an os process.

use std::io::Result;

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

/// This trait represents a platform independent process which runs a program.
pub trait Process: Sized {
    /// A command which process can run.
    type Command;
    /// A representation of IO stream of communication with a programm a process is running.
    type Stream;

    /// Spawn parses a given string as a commandline string and spawns it on a process.
    fn spawn<S: AsRef<str>>(cmd: S) -> Result<Self>;
    /// Spawn_command runs a process with a given command.
    fn spawn_command(command: Self::Command) -> Result<Self>;
    /// It opens a IO stream with a spawned process.
    fn open_stream(&mut self) -> Result<Self::Stream>;
}

/// Healthcheck represents a check by which we can determine if a spawned process is still alive.
pub trait Healthcheck {
    /// The function returns a status of a process if it still alive and it can operate.
    fn is_alive(&mut self) -> Result<bool>;
}

/// NonBlocking interface represens a [std::io::Read]er which can be turned in a non blocking mode
/// so its read operations will return imideately.
pub trait NonBlocking {
    /// Sets a [std::io::Read]er into a non blocking mode.
    fn set_non_blocking(&mut self) -> Result<()>;
    /// Sets a [std::io::Read]er back into a blocking mode.
    fn set_blocking(&mut self) -> Result<()>;
}

#[cfg(feature = "async")]
/// IntoAsyncStream interface turns a [Process::Stream] into an async version.
/// To be used with `async`/`await`syntax
pub trait IntoAsyncStream {
    /// AsyncStream type.
    /// Like [Process::Stream] but it represents an async IO stream.
    type AsyncsStream;

    /// Turns an object into a async stream.
    fn into_async_stream(self) -> Result<Self::AsyncsStream>;
}
