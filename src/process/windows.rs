use std::io::{self, Read, Result, Write};

use conpty::{PipeReader, PipeWriter, Process};

use super::{NonBlocking, Process};

pub struct WindowsProcess(Process);

impl WindowsProcess {
    fn spawn<S: AsRef<str>>(command: S) -> Result<Self> {
        Process::spawn(conpty::ProcAttr::cmd(command.as_ref().to_string())).map(WindowsProcess)
    }

    fn spawn_command(command: std::process::Command) -> Result<Self> {
        todo!("Can work on latest compiler")
    }
}

impl Process for WindowsProcess {
    type Stream = ProcessStream;

    fn stream(&mut self) -> Result<Self::Stream> {
        let input = self.0.input()?;
        let output = self.0.output()?;
        Ok(Self::Stream::new(input, output))
    }

    fn get_eof_char(&mut self) -> Result<char> {
        Ok(0x4)
    }

    fn get_intr_char(&mut self) -> Result<char> {
        Ok(0x3)
    }
}

#[derive(Debug)]
pub struct ProcessStream {
    pub input: PipeWriter,
    pub output: PipeReader,
}

impl ProcessStream {
    fn new(output: PiperReader, input: PipeWriter) -> Self {
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
        self.output.set_non_blocking_mode()
    }

    fn set_blocking(&mut self) -> io::Result<()> {
        self.output.set_blocking_mode()
    }
}

impl super::Stream for ProcessStream {}
