use std::{
    io::{self, Read, Result, Write},
    ops::{Deref, DerefMut},
};

use conpty::{
    io::{PipeReader, PipeWriter},
    ProcAttr, Process,
};

use super::Process as ProcessTrait;
use crate::session::stream::NonBlocking;

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
            .map_err(to_io_error)
            .map(|proc| WinProcess { proc })
    }

    fn open_stream(&mut self) -> Result<Self::Stream> {
        let input = self.proc.input().map_err(to_io_error)?;
        let output = self.proc.output().map_err(to_io_error)?;
        Ok(Self::Stream::new(output, input))
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
        self.output.set_non_blocking_mode().map_err(to_io_error)
    }

    fn set_blocking(&mut self) -> io::Result<()> {
        self.output.set_blocking_mode().map_err(to_io_error)
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

fn to_io_error(err: impl std::error::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err.to_string())
}
