use polling::{Event, Poller};
use std::{
    cmp,
    fs::File,
    io::{self, Read, Write},
    os::unix::prelude::AsRawFd,
    time::Duration,
};

pub struct BufStream {
    inner: Stream,
    buffer: Vec<u8>,
    eof: bool,
}

impl BufStream {
    pub fn new(stream: Stream) -> Self {
        Self {
            inner: stream,
            buffer: Vec::new(),
            eof: false,
        }
    }

    pub fn is_eof(&self) -> bool {
        self.eof
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.buffer.as_slice()
    }

    pub fn drain(&mut self, to: usize) {
        self.buffer.drain(..to);
    }

    pub fn try_fill(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        let mut buf = [0u8; 1024];
        let n = self.inner.try_read(&mut buf, timeout)?;
        if n == 0 {
            self.eof = true;
        } else {
            self.buffer.extend(&buf[..n]);
        }

        Ok(())
    }

    fn read_from_buffer(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count_can_read = cmp::min(buf.len(), self.buffer.len());
        let buffered = self.buffer.drain(..count_can_read);
        buffered.into_iter().enumerate().for_each(|(i, b)| {
            buf[i] = b;
        });

        Ok(count_can_read)
    }
}

impl Write for BufStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }
}

impl Read for BufStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.read_from_buffer(buf)?;
        self.inner.read(&mut buf[n..])
    }
}

pub struct Stream {
    file: File,
    poller: Poller,
}

impl Stream {
    pub fn new(file: File) -> io::Result<Self> {
        let poller = Poller::new()?;
        // Register interest in readability of the file.
        let key = file.as_raw_fd() as usize; // Arbitrary key identifying the file.
        poller.add(&file, Event::readable(key))?;

        let timeout = Some(Duration::from_millis(3000));

        Ok(Self { file, poller })
    }

    pub fn try_read(&mut self, buf: &mut [u8], timeout: Option<Duration>) -> io::Result<usize> {
        let key = self.file.as_raw_fd() as usize;
        self.poller.modify(&self.file, Event::readable(key))?;

        let mut events = Vec::new();
        let occured_events = self.poller.wait(&mut events, timeout)?;
        if occured_events == 0 {
            return Err(timeout_error());
        }

        self.read(buf)
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

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.file.write_vectored(bufs)
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

pub fn is_timeout_error(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::TimedOut && err.to_string() == "A timeout timer has been fired"
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
    fn blocking_io() {
        let (mut stream, _proc) = cat_stream();

        let message = b"Hello World";
        stream.write_all(message).unwrap();
        let mut buf = vec![0u8; message.len()];
        stream.read_exact(&mut buf).unwrap();

        assert_eq!(buf, b"Hello World");
    }

    #[test]
    fn no_blocking_io() {
        let (mut stream, _proc) = cat_stream();

        let message = b"Hello World";
        stream.write_all(message).unwrap();

        // sometimes the send data aren't processed
        // So to make sure that it is we sleep a bit
        std::thread::sleep(Duration::from_millis(300));

        let buf = try_read_n(&mut stream, message.len()).unwrap();
        assert_eq!(buf, b"Hello World");
        assert!(stream
            .try_read(&mut Vec::new(), Some(Duration::from_millis(100)))
            .is_err());
    }

    #[test]
    fn no_blocking_io_timeout_on_empty_stream() {
        let (mut stream, _proc) = cat_stream();

        assert!(stream
            .try_read(&mut Vec::new(), Some(Duration::from_millis(100)))
            .is_err());
    }

    fn try_read_n(stream: &mut Stream, n: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0; n];
        let read = stream.try_read(&mut buf, None).unwrap();

        assert_eq!(read, n);

        Ok(buf)
    }

    fn cat_stream() -> (Stream, PtyProcess) {
        let proc = PtyProcess::spawn(Command::new("cat")).unwrap();
        let master = proc.get_file_handle().unwrap();
        let stream = Stream::new(master).unwrap();

        (stream, proc)
    }
}
