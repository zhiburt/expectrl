//! This module contains a Windows implementation of [crate::process::Process].

use std::{
    io::{self, Read, Result, Write},
    ops::{Deref, DerefMut},
    process::Command,
};

use conpty::{
    io::{PipeReader, PipeWriter},
    spawn, Process,
};

use super::{Healthcheck, NonBlocking, Process as ProcessTrait};
use crate::error::to_io_error;

#[cfg(feature = "async")]
use super::IntoAsyncStream;
#[cfg(feature = "async")]
use futures_lite::{AsyncRead, AsyncWrite};
#[cfg(feature = "async")]
use std::{
    pin::Pin,
    task::{Context, Poll},
};

/// A windows representation of a [Process] via [conpty::Process].
#[derive(Debug)]
pub struct WinProcess {
    proc: Process,
}

impl ProcessTrait for WinProcess {
    type Command = Command;
    type Stream = ProcessStream;

    fn spawn<S: AsRef<str>>(cmd: S) -> Result<Self> {
        spawn(cmd.as_ref())
            .map_err(to_io_error(""))
            .map(|proc| WinProcess { proc })
    }

    fn spawn_command(command: Self::Command) -> Result<Self> {
        conpty::Process::spawn(command)
            .map_err(to_io_error(""))
            .map(|proc| WinProcess { proc })
    }

    fn open_stream(&mut self) -> Result<Self::Stream> {
        let input = self.proc.input().map_err(to_io_error(""))?;
        let output = self.proc.output().map_err(to_io_error(""))?;
        Ok(Self::Stream::new(output, input))
    }
}

impl Healthcheck for WinProcess {
    fn is_alive(&mut self) -> Result<bool> {
        Ok(self.proc.is_alive())
    }
}

impl Deref for WinProcess {
    type Target = Process;

    fn deref(&self) -> &Self::Target {
        &self.proc
    }
}

impl DerefMut for WinProcess {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.proc
    }
}

/// An IO stream of [WinProcess].
#[derive(Debug)]
pub struct ProcessStream {
    input: PipeWriter,
    output: PipeReader,
}

impl ProcessStream {
    fn new(output: PipeReader, input: PipeWriter) -> Self {
        Self { input, output }
    }

    /// Tries to clone the stream.
    pub fn try_clone(&self) -> std::result::Result<Self, conpty::error::Error> {
        Ok(Self {
            input: self.input.try_clone()?,
            output: self.output.try_clone()?,
        })
    }
}

impl Write for ProcessStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.input.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.input.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> Result<usize> {
        self.input.write_vectored(bufs)
    }
}

impl Read for ProcessStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.output.read(buf)
    }
}

impl NonBlocking for ProcessStream {
    fn set_non_blocking(&mut self) -> Result<()> {
        self.output.blocking(false);
        Ok(())
    }

    fn set_blocking(&mut self) -> Result<()> {
        self.output.blocking(true);
        Ok(())
    }
}

#[cfg(feature = "async")]
impl IntoAsyncStream for ProcessStream {
    type AsyncStream = AsyncProcessStream;

    fn into_async_stream(self) -> Result<Self::AsyncStream> {
        AsyncProcessStream::new(self)
    }
}

/// An async version of IO stream of [WinProcess].
#[cfg(feature = "async")]
#[derive(Debug)]
pub struct AsyncProcessStream {
    output: blocking::Unblock<PipeReader>,
    input: blocking::Unblock<PipeWriter>,
}

#[cfg(feature = "async")]
impl AsyncProcessStream {
    fn new(stream: ProcessStream) -> Result<Self> {
        let input = blocking::Unblock::new(stream.input);
        let output = blocking::Unblock::new(stream.output);
        Ok(Self { input, output })
    }
}

#[cfg(feature = "async")]
impl AsyncWrite for AsyncProcessStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.input).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.input).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.input).poll_close(cx)
    }
}

#[cfg(feature = "async")]
impl AsyncRead for AsyncProcessStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.output).poll_read(cx, buf)
    }
}
