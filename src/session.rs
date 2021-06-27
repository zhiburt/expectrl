/*
    - test why tests with new lines fails
    - expect set of calls
    - proc methods: kill wait etc.
*/

use futures_lite::{AsyncRead, AsyncReadExt, AsyncWrite};

use crate::{error::Error, process::PtyProcess, session_stream::Stream};
use std::{
    convert::TryFrom,
    fmt,
    fs::File,
    io::{self, BufReader, BufWriter, IoSlice, Read, Write},
    os::unix::prelude::{AsRawFd, FromRawFd},
    process::Command,
    task::Poll,
};

pub struct PtySession {
    proc: PtyProcess,
    master: Stream,
}

impl PtySession {
    pub fn spawn(cmd: &str) -> Result<Self, Error> {
        let command = build_command(cmd)?;
        let ptyproc = PtyProcess::spawn(command)?;
        let master = ptyproc.get_file_handle()?;
        let stream = Stream::new(master)?;

        Ok(Self {
            proc: ptyproc,
            master: stream,
        })
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

        assert_eq!(buf, "Hello World\n");

        Ok(())
    }

    #[test]
    fn send_line() -> Result<(), Error> {
        let mut session = PtySession::spawn("cat")?;
        let n = session.send_line("Hello World")?;

        println!("{}", n);

        thread::sleep(Duration::from_millis(300));
        session.write_all(&[3])?; // Ctrl+C
        session.flush()?;

        let mut buf = String::new();
        session.read_to_string(&mut buf)?;

        println!("{}", buf);

        #[cfg(windows)]
        {
            assert_eq!(buf, "Hello World\r\n");
        }
        #[cfg(not(windows))]
        {
            assert_eq!(buf, "Hello World\n");
        }

        Ok(())
    }
}
