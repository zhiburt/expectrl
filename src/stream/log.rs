//! A wrapper of [Session] to log a read/write operations
use std::{
    io::{self, Read, Result, Write},
    ops::{Deref, DerefMut},
};

use crate::session::stream::NonBlocking;

pub struct LoggedStream<W, S> {
    stream: S,
    logger: W,
}

impl<W: Write, S> LoggedStream<W, S> {
    pub fn new(stream: S, logger: W) -> Self {
        Self { stream, logger }
    }

    fn log_write(&mut self, buf: &[u8]) {
        log(&mut self.logger, "write", buf);
    }

    fn log_read(&mut self, buf: &[u8]) {
        log(&mut self.logger, "read", buf);
    }
}

impl<W: Write, S: Write> Write for LoggedStream<W, S> {
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

impl<S: Read, W: Write> Read for LoggedStream<W, S> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = self.stream.read(buf)?;
        self.log_read(&buf[..n]);
        Ok(n)
    }
}

impl<S: NonBlocking, W> NonBlocking for LoggedStream<W, S> {
    fn set_non_blocking(&mut self) -> Result<()> {
        self.stream.set_non_blocking()
    }

    fn set_blocking(&mut self) -> Result<()> {
        self.stream.set_blocking()
    }
}

impl<S, W> Deref for LoggedStream<W, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<S, W> DerefMut for LoggedStream<W, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

#[cfg(unix)]
impl<S: std::os::unix::prelude::AsRawFd, W> std::os::unix::prelude::AsRawFd for LoggedStream<W, S> {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.stream.as_raw_fd()
    }
}

#[cfg(windows)]
impl<S: std::os::windows::io::AsRawSocket, W> std::os::windows::io::AsRawSocket
    for LoggedStream<W, S>
{
    fn as_raw_socket(&self) -> std::os::windows::prelude::RawSocket {
        self.stream.as_raw_socket()
    }
}

fn log(mut writer: impl Write, target: &str, data: &[u8]) {
    let _ = match std::str::from_utf8(data) {
        Ok(data) => writeln!(writer, "{}: {:?}", target, data),
        Err(..) => writeln!(writer, "{}:(bytes): {:?}", target, data),
    };
}
