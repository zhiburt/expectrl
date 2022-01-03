use std::{
    io::{self, Read, Result, Write},
    ops::{Deref, DerefMut},
    os::unix::prelude::{AsRawFd, RawFd},
};

use ptyprocess::{stream::Stream, PtyProcess};

use super::Process;

pub struct UnixProcess(PtyProcess);

impl UnixProcess {
    pub fn spawn<S: AsRef<str>>(cmd: S) -> Result<Self> {
        let args = tokenize_command(cmd.as_ref());
        if args.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a commandline argument is not correct",
            ));
        }

        let mut command = std::process::Command::new(&args[0]);
        command.args(args.iter().skip(1));

        Self::spawn_command(command)
    }

    pub fn spawn_command(command: std::process::Command) -> Result<Self> {
        PtyProcess::spawn(command)
            .map(UnixProcess)
            .map_err(nix_error_to_io)
    }
}

#[cfg(not(feature = "async"))]
impl Process for UnixProcess {
    type Stream = PtyStream;

    fn stream(&mut self) -> std::io::Result<Self::Stream> {
        let handle_stream = self.0.get_pty_stream().map_err(nix_error_to_io)?;
        Ok(Self::Stream::new(handle_stream))
    }
}

#[cfg(feature = "async")]
impl Process for UnixProcess {
    type Stream = AsyncStream<PtyStream>;

    fn stream(&mut self) -> std::io::Result<Self::Stream> {
        let handle_stream = self.0.get_pty_stream().map_err(nix_error_to_io)?;
        let handle_stream = PtyStream::new(handle_stream);
        let handle_stream = AsyncStream::new(handle_stream)?;
        Ok(handle_stream)
    }
}

impl Deref for UnixProcess {
    type Target = PtyProcess;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for UnixProcess {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

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

#[cfg(not(feature = "async"))]
impl<A: AsRawFd> super::NonBlocking for A {
    fn set_non_blocking(&mut self) -> Result<()> {
        let fd = self.as_raw_fd();
        _make_non_blocking(fd, true)
    }

    fn set_blocking(&mut self) -> Result<()> {
        let fd = self.as_raw_fd();
        _make_non_blocking(fd, false)
    }
}

fn _make_non_blocking(fd: RawFd, blocking: bool) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
