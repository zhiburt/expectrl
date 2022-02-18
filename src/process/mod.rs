use std::io::Result;

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

pub trait Healthcheck {
    fn is_alive(&mut self) -> Result<bool>;
}

pub trait NonBlocking {
    fn set_non_blocking(&mut self) -> Result<()>;
    fn set_blocking(&mut self) -> Result<()>;
}

#[cfg(feature = "async")]
pub trait IntoAsyncStream {
    type AsyncsStream;

    fn into_async_stream(self) -> Result<Self::AsyncsStream>;
}
