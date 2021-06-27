use nix::poll;
use polling::{Event, Poller};
use std::{
    fs::File,
    io::{self, Read, Write},
    os::unix::prelude::AsRawFd,
    task::Poll,
    time::Duration,
};

use crate::error::Error;

pub struct Stream {
    file: File,
    poller: Poller,
    timeout: Option<Duration>,
}

impl Stream {
    pub fn new(file: File) -> io::Result<Self> {
        let poller = Poller::new()?;
        // Register interest in readability of the file.
        let key = file.as_raw_fd() as usize; // Arbitrary key identifying the file.
        poller.add(&file, Event::readable(key))?;

        let timeout = Some(Duration::from_millis(3000));

        Ok(Self {
            file,
            poller,
            timeout,
        })
    }

    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }

    pub fn try_read(&mut self) -> io::Result<u8> {
        let key = self.file.as_raw_fd() as usize;
        self.poller.modify(&self.file, Event::readable(key))?;

        let mut events = Vec::new();
        let occured_events = self.poller.wait(&mut events, self.timeout)?;
        if occured_events == 0 {
            return Err(timeout_error());
        }

        let mut buf = [0u8; 1];
        self.read_exact(&mut buf)?;

        Ok(buf[0])
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.poller.delete(&self.file);
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.file.read(buf) {
            Err(ref err) if has_reached_end_of_sdtout(err) => Ok(0),
            result => result,
        }
    }
}

fn timeout_error() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "A timeout timer has been fired")
}

fn has_reached_end_of_sdtout(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::Other && err.raw_os_error() == Some(5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::PtyProcess;
    use std::process::Command;

    #[test]
    fn blocking_io() -> Result<(), Error> {
        let mut stream = cat_stream()?;

        let message = b"Hello World";
        stream.write_all(message)?;
        let mut buf = vec![0u8; message.len()];
        stream.read_exact(&mut buf)?;

        assert_eq!(buf, b"Hello World");

        Ok(())
    }

    #[test]
    fn no_blocking_io() -> Result<(), Error> {
        let mut stream = cat_stream().unwrap();
        stream.set_timeout(Some(Duration::from_millis(100)));

        let message = b"Hello World";
        stream.write_all(message).unwrap();

        // sometimes the send data aren't processed
        // So to make sure that it is we sleep a bit
        std::thread::sleep(Duration::from_millis(300));

        let buf = try_read_n(&mut stream, message.len())?;
        assert_eq!(buf, b"Hello World");
        assert!(stream.try_read().is_err());

        Ok(())
    }

    #[test]
    fn no_blocking_io_timeout_on_empty_stream() -> Result<(), Error> {
        let mut stream = cat_stream().unwrap();
        stream.set_timeout(Some(Duration::from_millis(100)));

        assert!(stream.try_read().is_err());

        Ok(())
    }

    fn try_read_n(stream: &mut Stream, n: usize) -> io::Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(n);
        for _ in 0..n {
            let byte = stream.try_read()?;
            buf.push(byte);
        }

        Ok(buf)
    }

    fn cat_stream() -> Result<Stream, Error> {
        let proc = PtyProcess::spawn(Command::new("cat"))?;
        let master = proc.get_file_handle()?;
        let stream = Stream::new(master)?;

        Ok(stream)
    }
}
