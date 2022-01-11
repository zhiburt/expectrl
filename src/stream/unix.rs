use std::{
    io::{self, Read, Result, Write},
    os::unix::prelude::{AsRawFd, RawFd},
};

use ptyprocess::stream::Stream;

use super::stream::NonBlocking;


#[derive(Debug)]
pub struct PtyStream {
    handle: Stream,
}

impl PtyStream {
    pub fn new(stream: Stream) -> Self {
        Self { handle: stream }
    }
}

impl Write for PtyStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.handle.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.handle.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> Result<usize> {
        self.handle.write_vectored(bufs)
    }
}

impl Read for PtyStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.handle.read(buf)
    }
}

impl AsRawFd for PtyStream {
    fn as_raw_fd(&self) -> RawFd {
        self.handle.as_raw_fd()
    }
}

impl NonBlocking for PtyStream {
    fn set_non_blocking(&mut self) -> Result<()> {
        let fd = self.as_raw_fd();
        _make_non_blocking(fd, true)
    }

    fn set_blocking(&mut self) -> Result<()> {
        let fd = self.as_raw_fd();
        _make_non_blocking(fd, false)
    }
}

pub fn _make_non_blocking(fd: RawFd, blocking: bool) -> Result<()> {
    use nix::fcntl::{fcntl, FcntlArg, OFlag};

    let opt = fcntl(fd, FcntlArg::F_GETFL).map_err(nix_error_to_io)?;
    let mut opt = OFlag::from_bits_truncate(opt);
    opt.set(OFlag::O_NONBLOCK, blocking);
    fcntl(fd, FcntlArg::F_SETFL(opt)).map_err(nix_error_to_io)?;
    Ok(())
}

fn nix_error_to_io(err: nix::Error) -> io::Error {
    match err.as_errno() {
        Some(code) => io::Error::from_raw_os_error(code as _),
        None => io::Error::new(
            io::ErrorKind::Other,
            "Unexpected error type conversion from nix to io",
        ),
    }
}
