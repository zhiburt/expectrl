use super::Process;
use crate::session::stream::NonBlocking;
use ptyprocess::{stream::Stream, PtyProcess};
use std::{
    io::{self, Read, Result, Write},
    ops::{Deref, DerefMut},
    os::unix::prelude::{AsRawFd, RawFd},
    process::Command,
};

pub struct UnixProcess {
    proc: PtyProcess,
}

impl Process for UnixProcess {
    type Command = Command;
    type Session = PtyStream;

    fn spawn<S: AsRef<str>>(cmd: S) -> Result<Self> {
        let args = tokenize_command(cmd.as_ref());
        if args.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to parse a command",
            ));
        }

        let mut command = std::process::Command::new(&args[0]);
        command.args(args.iter().skip(1));

        Self::spawn_command(command)
    }

    fn spawn_command(command: Self::Command) -> Result<Self> {
        let proc = PtyProcess::spawn(command).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to spawn a command; {}", e),
            )
        })?;

        Ok(Self { proc })
    }

    fn open_session(&mut self) -> Result<Self::Session> {
        let stream = self.proc.get_pty_stream().map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to create a stream; {}", e),
            )
        })?;
        let stream = PtyStream::new(stream);
        Ok(stream)
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
#[cfg(unix)]
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
