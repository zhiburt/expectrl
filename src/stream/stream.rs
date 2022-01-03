//! Module responsible for IO operations of [crate::Session].
//!
//! Module contains [Stream] structure async and sync flow and different one for windows.
//! It also contains a [ReaderWithBuffer] for controlling buffering.

use std::io::{self, BufRead, BufReader, Read, Write};

use crate::process::NonBlocking;

#[derive(Debug)]
pub struct TryStream<S> {
    stream: ControlledReader<S>,
}

impl<S: Read> TryStream<S> {
    /// The function returns a new Stream from a file.
    pub fn new(stream: S) -> io::Result<Self> {
        Ok(Self {
            stream: ControlledReader::new(stream),
        })
    }
}

impl<S: Read> TryStream<S> {
    fn from_stream<N: Read>(&mut self, stream: N) -> io::Result<TryStream<N>> {
        self.stream.flush_in_buffer();
        let buffer = self.stream.get_available();
        let mut stream = TryStream::new(stream)?;
        stream.stream.keep_in_buffer(buffer);
        Ok(stream)
    }

    pub fn swap_stream<N: Read>(mut self, stream: N) -> io::Result<(TryStream<N>, S)> {
        let new = self.from_stream(stream)?;
        let old = self.stream.inner.into_inner().inner;
        Ok((new, old))
    }

    pub fn get_available(&mut self) -> &[u8] {
        self.stream.get_available()
    }

    pub fn consume_available(&mut self, n: usize) {
        self.stream.consume_available(n)
    }
}

impl<R: Read + NonBlocking> TryStream<R> {
    /// Try to read in a non-blocking mode.
    ///
    /// It raises io::ErrorKind::WouldBlock if there's nothing to read.
    pub fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.get_mut().set_non_blocking()?;

        let result = match self.stream.inner.read(buf) {
            Ok(n) => Ok(n),
            Err(err) => Err(err),
        };

        // As file is DUPed changes in one descriptor affects all ones
        // so we need to make blocking file after we finished.
        self.stream.get_mut().set_blocking()?;

        result
    }

    pub fn is_empty(&mut self) -> io::Result<bool> {
        match self.try_read(&mut []) {
            Ok(0) => Ok(true),
            Ok(_) => Ok(false),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(true),
            Err(err) => Err(err),
        }
    }

    pub fn read_available(&mut self) -> std::io::Result<bool> {
        self.stream.flush_in_buffer();

        let mut buf = [0; 248];
        loop {
            match self.try_read_inner(&mut buf) {
                Ok(0) => break Ok(true),
                Ok(n) => {
                    self.stream.keep_in_buffer(&buf[..n]);
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break Ok(false),
                Err(err) => break Err(err),
            }
        }
    }

    pub fn read_available_once(&mut self, buf: &mut [u8]) -> std::io::Result<Option<usize>> {
        self.stream.flush_in_buffer();

        match self.try_read_inner(buf) {
            Ok(0) => Ok(Some(0)),
            Ok(n) => {
                self.stream.keep_in_buffer(&buf[..n]);
                Ok(Some(n))
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(err),
        }
    }

    // non-buffered && non-blocking read
    fn try_read_inner(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.get_mut().set_non_blocking()?;

        let result = match self.stream.get_mut().read(buf) {
            Ok(n) => Ok(n),
            Err(err) => Err(err),
        };

        // As file is DUPed changes in one descriptor affects all ones
        // so we need to make blocking file after we finished.
        self.stream.get_mut().set_blocking()?;

        result
    }
}

impl<S: Write> Write for TryStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.inner.get_mut().inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.inner.get_mut().inner.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.stream.inner.get_mut().inner.write_vectored(bufs)
    }
}

impl<R: Read> Read for TryStream<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.inner.read(buf)
    }
}

impl<R: Read> BufRead for TryStream<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.stream.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.stream.inner.consume(amt)
    }
}

#[derive(Debug)]
pub struct ControlledReader<R> {
    inner: BufReader<BufferedReader<R>>,
}

impl<R: Read> ControlledReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: BufReader::new(BufferedReader::new(reader)),
        }
    }

    pub fn keep_in_buffer(&mut self, v: &[u8]) {
        self.inner.get_mut().buffer.extend(v);
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner.get_mut().inner
    }

    pub fn get_available(&mut self) -> &[u8] {
        &self.inner.get_ref().buffer
    }

    pub fn consume_available(&mut self, n: usize) {
        self.inner.get_mut().buffer.drain(..n);
    }

    pub fn flush_in_buffer(&mut self) {
        // Because we have 2 buffered streams there might appear inconsistancy
        // in read operations and the data which was via `keep_in_buffer` function.
        //
        // To eliminate it we move BufReader buffer to our buffer.
        let b = self.inner.buffer().to_vec();
        self.inner.consume(b.len());
        self.keep_in_buffer(&b);
    }
}

#[derive(Debug)]
struct BufferedReader<R> {
    inner: R,
    buffer: Vec<u8>,
}

impl<R> BufferedReader<R> {
    fn new(reader: R) -> Self {
        Self {
            inner: reader,
            buffer: Vec::new(),
        }
    }
}

impl<R: Read> Read for BufferedReader<R> {
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        if self.buffer.is_empty() {
            self.inner.read(buf)
        } else {
            let n = buf.write(&self.buffer)?;
            self.buffer.drain(..n);
            Ok(n)
        }
    }
}
