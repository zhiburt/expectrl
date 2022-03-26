use super::{Healthcheck, NonBlocking, Process};
use crate::error::to_io_error;
use ptyprocess::{stream::Stream, PtyProcess};

#[cfg(feature = "async")]
use super::IntoAsyncStream;
#[cfg(feature = "async")]
use futures_lite::{AsyncRead, AsyncWrite};
#[cfg(feature = "async")]
use std::{
    pin::Pin,
    task::{Context, Poll},
};

use std::{
    io::{self, Read, Result, Write},
    ops::{Deref, DerefMut},
    os::unix::prelude::{AsRawFd, RawFd},
    process::Command,
};

/// A Unix representation of a [Process] via [PtyProcess]
pub struct UnixProcess {
    proc: PtyProcess,
}

impl Process for UnixProcess {
    type Command = Command;
    type Stream = PtyStream;

    fn spawn<S: AsRef<str>>(cmd: S) -> Result<Self> {
        let args = tokenize_command(cmd.as_ref());
        if args.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to parse a command",
            ));
        }

        let mut command = std::process::Command::new(&args[0]);
        let _ = command.args(args.iter().skip(1));

        Self::spawn_command(command)
    }

    fn spawn_command(command: Self::Command) -> Result<Self> {
        let proc = PtyProcess::spawn(command).map_err(to_io_error("Failed to spawn a command"))?;

        Ok(Self { proc })
    }

    fn open_stream(&mut self) -> Result<Self::Stream> {
        let stream = self
            .proc
            .get_pty_stream()
            .map_err(to_io_error("Failed to create a stream"))?;
        let stream = PtyStream::new(stream);
        Ok(stream)
    }
}

impl Healthcheck for UnixProcess {
    fn is_alive(&mut self) -> Result<bool> {
        self.proc
            .is_alive()
            .map_err(to_io_error("Failed to call pty.is_alive()"))
    }
}

impl Deref for UnixProcess {
    type Target = PtyProcess;

    fn deref(&self) -> &Self::Target {
        &self.proc
    }
}

impl DerefMut for UnixProcess {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.proc
    }
}

/// A IO stream (write/read) of [UnixProcess].
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

impl NonBlocking for PtyStream {
    fn set_non_blocking(&mut self) -> Result<()> {
        let fd = self.handle.as_raw_fd();
        _make_non_blocking(fd, true)
    }

    fn set_blocking(&mut self) -> Result<()> {
        let fd = self.handle.as_raw_fd();
        _make_non_blocking(fd, false)
    }
}

pub(crate) fn _make_non_blocking(fd: RawFd, blocking: bool) -> Result<()> {
    use nix::fcntl::{fcntl, FcntlArg, OFlag};

    let opt = fcntl(fd, FcntlArg::F_GETFL).map_err(nix_error_to_io)?;
    let mut opt = OFlag::from_bits_truncate(opt);
    opt.set(OFlag::O_NONBLOCK, blocking);
    let _ = fcntl(fd, FcntlArg::F_SETFL(opt)).map_err(nix_error_to_io)?;
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

/// Turn e.g. "prog arg1 arg2" into ["prog", "arg1", "arg2"]
/// It takes care of single and double quotes but,
///
/// It doesn't cover all edge cases.
/// So it may not be compatible with real shell arguments parsing.
fn tokenize_command(program: &str) -> Vec<String> {
    let re = regex::Regex::new(r#""[^"]+"|'[^']+'|[^'" ]+"#).unwrap();
    let mut res = vec![];
    for cap in re.captures_iter(program) {
        res.push(cap[0].to_string());
    }
    res
}

#[cfg(feature = "async")]
impl IntoAsyncStream for PtyStream {
    type AsyncsStream = AsyncPtyStream;

    fn into_async_stream(self) -> Result<Self::AsyncsStream> {
        AsyncPtyStream::new(self)
    }
}

impl AsRawFd for PtyStream {
    fn as_raw_fd(&self) -> RawFd {
        self.handle.as_raw_fd()
    }
}

/// An async version of IO stream of [UnixProcess].
#[cfg(feature = "async")]
pub struct AsyncPtyStream {
    stream: async_io::Async<PtyStream>,
}

#[cfg(feature = "async")]
impl AsyncPtyStream {
    pub fn new(stream: PtyStream) -> Result<Self> {
        let stream = async_io::Async::new(stream)?;
        Ok(Self { stream })
    }
}

#[cfg(feature = "async")]
impl AsyncWrite for AsyncPtyStream {
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
impl AsyncRead for AsyncPtyStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn test_tokenize_command() {
        let res = tokenize_command("prog arg1 arg2");
        assert_eq!(vec!["prog", "arg1", "arg2"], res);

        let res = tokenize_command("prog -k=v");
        assert_eq!(vec!["prog", "-k=v"], res);

        let res = tokenize_command("prog 'my text'");
        assert_eq!(vec!["prog", "'my text'"], res);

        let res = tokenize_command(r#"prog "my text""#);
        assert_eq!(vec!["prog", r#""my text""#], res);
    }
}
