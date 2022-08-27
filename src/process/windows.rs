//! This module contains a Windows implementation of [crate::process::Process].

use std::{
    collections::HashMap,
    ffi::OsStr,
    io::{self, Read, Result, Write},
    iter::FromIterator,
    ops::{Deref, DerefMut},
    process::Command,
};

use conpty::{
    io::{PipeReader, PipeWriter},
    ProcAttr, Process,
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
pub struct WinProcess {
    proc: Process,
}

impl ProcessTrait for WinProcess {
    type Command = Command;
    type Stream = ProcessStream;

    fn spawn<S: AsRef<str>>(cmd: S) -> Result<Self> {
        Self::spawn_command(Command::new(cmd.as_ref()))
    }

    fn spawn_command(command: Self::Command) -> Result<Self> {
        let attr = command_to_proc_attr(command)
            .ok_or_else(|| to_io_error("command parsing error")(""))?;
        attr.spawn()
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
        self.output.set_non_blocking_mode().map_err(to_io_error(""))
    }

    fn set_blocking(&mut self) -> Result<()> {
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

/// An async version of IO stream of [WinProcess].
#[cfg(feature = "async")]
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

fn command_to_proc_attr(cmd: Command) -> Option<ProcAttr> {
    let program = cmd.get_program().to_str()?;
    let mut attr = ProcAttr::cmd(program);

    if let Some(dir) = cmd.get_current_dir() {
        let dir = dir.to_str()?;

        attr = attr.current_dir(dir);
    }

    if cmd.get_args().len() > 0 {
        let args = cmd
            .get_args()
            .into_iter()
            .map(os_str_to_string)
            .collect::<Option<Vec<String>>>()?;

        attr = attr.args(args);
    }

    if cmd.get_envs().len() > 0 {
        let envs = cmd
            .get_envs()
            .into_iter()
            .filter(|(_, v)| v.is_some())
            .map(|(k, v)| {
                os_str_to_string(k).and_then(|k| os_str_to_string(v.unwrap()).map(|v| (k, v)))
            })
            .collect::<Option<Vec<(String, String)>>>()?;
        let envs = HashMap::from_iter(envs);

        attr = attr.envs(envs);
    }

    Some(attr)
}

fn os_str_to_string(s: &OsStr) -> Option<String> {
    s.to_str().map(|s| s.to_owned())
}
