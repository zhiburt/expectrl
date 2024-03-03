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
    fn spawn<S>(cmd: S) -> Result<Self>
    where
        S: AsRef<str>;
    /// Spawn_command runs a process with a given command.
    fn spawn_command(command: Self::Command) -> Result<Self>;
    /// It opens a IO stream with a spawned process.
    fn open_stream(&mut self) -> Result<Self::Stream>;
}

#[allow(clippy::wrong_self_convention)]
/// Healthcheck represents a check by which we can determine if a spawned process is still alive.
pub trait Healthcheck {
    /// A status healthcheck can return.
    type Status;

    /// The function returns a status of a process if it still alive and it can operate.
    fn get_status(&self) -> Result<Self::Status>;

    /// The function returns a status of a process if it still alive and it can operate.
    fn is_alive(&self) -> Result<bool>;
}

impl<T> Healthcheck for &T
where
    T: Healthcheck,
{
    type Status = T::Status;

    fn get_status(&self) -> Result<Self::Status> {
        T::get_status(self)
    }

    fn is_alive(&self) -> Result<bool> {
        T::is_alive(self)
    }
}

impl<T> Healthcheck for &mut T
where
    T: Healthcheck,
{
    type Status = T::Status;

    fn get_status(&self) -> Result<Self::Status> {
        T::get_status(self)
    }

    fn is_alive(&self) -> Result<bool> {
        T::is_alive(self)
    }
}

/// NonBlocking interface represens a [std::io::Read]er which can be turned in a non blocking mode
/// so its read operations will return imideately.
pub trait NonBlocking {
    /// Sets a [std::io::Read]er into a non/blocking mode.
    fn set_blocking(&mut self, on: bool) -> Result<()>;
}

impl<T> NonBlocking for &mut T
where
    T: NonBlocking,
{
    fn set_blocking(&mut self, on: bool) -> Result<()> {
        T::set_blocking(self, on)
    }
}

/// Terminal configuration trait, used for IO configuration.
pub trait Termios {
    /// Verifies whether a [`std::io::Write`] will be repeated in output stream and be read by [`std::io::Read`].
    fn is_echo(&self) -> Result<bool>;
    /// Configure a echo logic.
    fn set_echo(&mut self, on: bool) -> Result<bool>;
}

impl<T> Termios for &mut T
where
    T: Termios,
{
    fn is_echo(&self) -> Result<bool> {
        T::is_echo(self)
    }

    fn set_echo(&mut self, on: bool) -> Result<bool> {
        T::set_echo(self, on)
    }
}

#[cfg(feature = "async")]
/// IntoAsyncStream interface turns a [Process::Stream] into an async version.
/// To be used with `async`/`await`syntax
pub trait IntoAsyncStream {
    /// AsyncStream type.
    /// Like [Process::Stream] but it represents an async IO stream.
    type AsyncStream;

    /// Turns an object into a async stream.
    fn into_async_stream(self) -> Result<Self::AsyncStream>;
}
