use std::{
    io::{self, Read, Result, Write},
    ops::{Deref, DerefMut},
};

use conpty::{
    io::{PipeReader, PipeWriter},
    ProcAttr, Process,
};

use super::{Healthcheck, Process as ProcessTrait};
use crate::error::to_io_error;
use crate::session::sync_stream::NonBlocking;

#[cfg(feature = "async")]
use super::IntoAsyncStream;
#[cfg(feature = "async")]
use futures_lite::{AsyncRead, AsyncWrite};
#[cfg(feature = "async")]
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pub struct WinProcess {
    proc: Process,
}

impl ProcessTrait for WinProcess {
    type Command = ProcAttr;

    type Stream = ProcessStream;

    fn spawn<S: AsRef<str>>(cmd: S) -> Result<Self> {
        Self::spawn_command(ProcAttr::cmd(cmd.as_ref()))
    }

    fn spawn_command(command: Self::Command) -> Result<Self> {
        command
            .spawn()
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

#[derive(Debug)]
pub struct ProcessStream {
    pub input: PipeWriter,
    pub output: PipeReader,
}

impl ProcessStream {
    pub fn new(output: PipeReader, input: PipeWriter) -> Self {
        Self { input, output }
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
    fn set_non_blocking(&mut self) -> io::Result<()> {
        self.output.set_non_blocking_mode().map_err(to_io_error(""))
    }

    fn set_blocking(&mut self) -> io::Result<()> {
        self.output.set_blocking_mode().map_err(to_io_error(""))
    }
}

#[cfg(feature = "async")]
impl IntoAsyncStream for ProcessStream {
    type AsyncsStream = AsyncProcessStream;

    fn into_async_stream(self) -> Result<Self::AsyncsStream> {
        AsyncProcessStream::new(self)
    }
}

#[cfg(feature = "async")]
pub struct AsyncProcessStream {
    stream: blocking::Unblock<ProcessStream>,
}

#[cfg(feature = "async")]
impl AsyncProcessStream {
    pub fn new(stream: ProcessStream) -> Result<Self> {
        let stream = blocking::Unblock::new(stream);
        Ok(Self { stream })
    }
}

#[cfg(feature = "async")]
impl AsyncWrite for AsyncProcessStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.stream).poll_close(cx)
    }
}

#[cfg(feature = "async")]
impl AsyncRead for AsyncProcessStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}
