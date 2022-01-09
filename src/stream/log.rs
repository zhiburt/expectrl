//! A wrapper of [Session] to log a read/write operations
use std::{
    io::{self, Read, Result, Write},
    ops::{Deref, DerefMut},
};

use crate::process::NonBlocking;

pub struct LoggedStream<S, L> {
    stream: S,
    logger: L,
}

impl<S, L: Write> LoggedStream<S, L> {
    pub fn new(stream: S, logger: L) -> Self {
        Self { stream, logger }
    }

    fn log_write(&mut self, buf: &[u8]) {
        log(&mut self.logger, "write", buf);
    }

    fn log_read(&mut self, buf: &[u8]) {
        log(&mut self.logger, "read", buf);
    }
}

impl<S: Write, L: Write> Write for LoggedStream<S, L> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let n = self.stream.write(buf)?;
        self.log_write(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> Result<()> {
        self.stream.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> Result<usize> {
        let n = self.stream.write_vectored(bufs)?;

        let mut rest = n;
        let mut bytes = Vec::new();
        for buf in bufs {
            let written = std::cmp::min(buf.len(), rest);
            rest -= written;

            bytes.extend(&buf.as_ref()[..written]);

            if rest == 0 {
                break;
            }
        }

        self.log_write(&bytes);

        Ok(n)
    }
}

impl<S: Read, L: Write> Read for LoggedStream<S, L> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = self.stream.read(buf)?;
        self.log_read(&buf[..n]);
        Ok(n)
    }
}

impl<S, L> Deref for LoggedStream<S, L> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<S, L> DerefMut for LoggedStream<S, L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

impl<S: NonBlocking, L> NonBlocking for LoggedStream<S, L> {
    fn set_non_blocking(&mut self) -> Result<()> {
        self.stream.set_non_blocking()
    }

    fn set_blocking(&mut self) -> Result<()> {
        self.stream.set_blocking()
    }
}

fn log(mut writer: impl Write, target: &str, data: &[u8]) {
    let _ = match std::str::from_utf8(data) {
        Ok(data) => writeln!(writer, "{}: {:?}", target, data),
        Err(..) => writeln!(writer, "{}:(bytes): {:?}", target, data),
    };
}
