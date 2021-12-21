use std::{io::{self, Read, Result, Write}, ops::{Deref, DerefMut}};

use conpty::{io::{PipeWriter, PipeReader}, Process, ProcAttr};

use super::{NonBlocking, Process as ProcessTrait};

pub struct WindowsProcess(Process);

impl WindowsProcess {
    pub fn spawn<S: AsRef<str>>(command: S) -> Result<Self> {
        Self::spawn_command(ProcAttr::cmd(command.as_ref().to_string()))
    }

    pub fn spawn_command(command: ProcAttr) -> Result<Self> {
        command.spawn().map_err(to_io_error).map(WindowsProcess)
    }
}

impl ProcessTrait for WindowsProcess {
    type Stream = ProcessStream;

    fn stream(&mut self) -> Result<Self::Stream> {
        let input = self.0.input().map_err(to_io_error)?;
        let output = self.0.output().map_err(to_io_error)?;
        Ok(Self::Stream::new(output, input))
    }

    fn get_eof_char(&mut self) -> Result<u8> {
        Ok(0x4)
    }

    fn get_intr_char(&mut self) -> Result<u8> {
        Ok(0x3)
    }
}

#[derive(Debug)]
pub struct ProcessStream {
    pub input: PipeWriter,
    pub output: PipeReader,
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
    fn set_non_blocking(&mut self) -> io::Result<()> {
        self.output.set_non_blocking_mode().map_err(to_io_error)
    }

    fn set_blocking(&mut self) -> io::Result<()> {
        self.output.set_blocking_mode().map_err(to_io_error)
    }
}

impl super::Stream for ProcessStream {}

impl Deref for WindowsProcess {
    type Target = Process;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WindowsProcess {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn to_io_error(err: impl std::error::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err.to_string())
}