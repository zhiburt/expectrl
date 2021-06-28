/*
    - test why tests with new lines fails
    - expect set of calls
    - proc methods: kill wait etc.
*/

use nix::sys::wait::WaitStatus;

use crate::{
    error::Error,
    expect::Expect,
    process::PtyProcess,
    stream::{is_timeout_error, BufStream, Stream},
};
use std::{
    convert::TryFrom,
    fmt,
    fs::File,
    io::{self, BufReader, BufWriter, IoSlice, Read, Write},
    os::unix::prelude::{AsRawFd, FromRawFd},
    process::Command,
    task::Poll,
    thread,
    time::{self, Duration},
};

pub struct PtySession {
    proc: PtyProcess,
    master: BufStream,
    timeout: Option<Duration>,
}

impl PtySession {
    pub fn spawn(cmd: &str) -> Result<Self, Error> {
        let command = build_command(cmd)?;
        let ptyproc = PtyProcess::spawn(command)?;
        let master = ptyproc.get_file_handle()?;
        let stream = BufStream::new(Stream::new(master)?);

        Ok(Self {
            proc: ptyproc,
            master: stream,
            timeout: Some(Duration::from_millis(10000)),
        })
    }

    pub fn expect<E: Expect>(&mut self, expect: E) -> Result<E::Output, Error> {
        let start = time::Instant::now();
        loop {
            self.master.try_fill(self.timeout)?;

            let buf = self.master.as_bytes();
            let eof = self.master.is_eof();

            if let Some((out, m)) = expect.expect(&buf, eof) {
                self.master.drain(m.end());
                return Ok(out);
            }

            if let Some(timeout) = self.timeout {
                if start.elapsed() > timeout {
                    return Err(Error::ExpectTimeout);
                }
            }
        }
    }

    pub fn send<S: AsRef<str>>(&mut self, str: S) -> Result<usize, Error> {
        let n = self.write(str.as_ref().as_bytes())?;
        self.flush()?;
        Ok(n)
    }

    pub fn send_line<S: AsRef<str>>(&mut self, str: S) -> Result<usize, Error> {
        #[cfg(windows)]
        const LINE_ENDING: &[u8] = b"\r\n";
        #[cfg(not(windows))]
        const LINE_ENDING: &[u8] = b"\n";

        let bufs = &mut [
            IoSlice::new(str.as_ref().as_bytes()),
            IoSlice::new(LINE_ENDING),
        ];

        let n = self.write_vectored(bufs)?;
        self.flush()?;

        Ok(n)
    }

    pub fn exit(&mut self) -> Result<(), Error> {
        self.proc.exit()?;
        Ok(())
    }

    pub fn wait(&mut self) -> Result<WaitStatus, Error> {
        let status = self.proc.wait()?;
        Ok(status)
    }
}

impl Write for PtySession {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.master.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.master.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.master.write_vectored(bufs)
    }
}

impl Read for PtySession {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.master.read(buf)
    }
}

fn build_command(cmd: &str) -> Result<Command, Error> {
    let mut args = cmd.split_whitespace();
    let bin = args.next().ok_or(Error::CommandParsing)?;

    let mut cmd = Command::new(bin);
    cmd.args(args);

    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    #[test]
    fn send() -> Result<(), Error> {
        let mut session = PtySession::spawn("cat")?;
        session.send("Hello World")?;

        thread::sleep(Duration::from_millis(300));
        session.write_all(&[3])?; // Ctrl+C
        session.flush()?;

        let mut buf = String::new();
        session.read_to_string(&mut buf)?;

        assert_eq!(buf, "Hello World");

        Ok(())
    }

    #[test]
    fn send_multiline() -> Result<(), Error> {
        let mut session = PtySession::spawn("cat")?;
        session.send("Hello World\n")?;

        thread::sleep(Duration::from_millis(300));
        session.write_all(&[3])?; // Ctrl+C
        session.flush()?;

        let mut buf = String::new();
        session.read_to_string(&mut buf)?;

        // cat repeats a send line after <enter> is presend
        // <enter> is basically a new line
        assert_eq!(buf, "Hello World\r\nHello World\r\n");

        Ok(())
    }

    #[test]
    fn send_line() -> Result<(), Error> {
        let mut session = PtySession::spawn("cat")?;
        let n = session.send_line("Hello World")?;

        #[cfg(windows)]
        {
            assert_eq!(n, 11 + 2);
        }
        #[cfg(not(windows))]
        {
            assert_eq!(n, 11 + 1);
        }

        thread::sleep(Duration::from_millis(300));
        session.exit()?;

        let mut buf = String::new();
        session.read_to_string(&mut buf)?;

        // cat repeats a send line after <enter> is presend
        // <enter> is basically a new line
        //
        // NOTE: in debug mode though it equals 'Hello World\r\n'
        // : stty -a are the same
        assert_eq!(buf, "Hello World\r\nHello World\r\n");

        Ok(())
    }

    #[test]
    fn expect_str() {
        let mut session = PtySession::spawn("cat").unwrap();
        session.send("Hello World").unwrap();
        session.expect("Hello World").unwrap();
    }

    #[test]
    fn read_after_expect_str() {
        let mut session = PtySession::spawn("cat").unwrap();
        session.send("Hello World").unwrap();
        session.expect("Hello").unwrap();

        let mut buf = [0; 6];
        session.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b" World");
    }
}