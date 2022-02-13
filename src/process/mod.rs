use std::io::{self, Read, Result, Write};

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

pub trait Process: Sized {
    type Command;
    type Stream;

    fn spawn<S: AsRef<str>>(cmd: S) -> Result<Self>;
    fn spawn_command(command: Self::Command) -> Result<Self>;
    fn open_stream(&mut self) -> Result<Self::Stream>;
}
