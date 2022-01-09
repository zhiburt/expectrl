// use std::{
//     io::{self, Read, Write},
//     os::unix::prelude::AsRawFd,
//     pin::Pin,
//     task::{Context, Poll},
// };

// use async_io::Async;
// use futures_lite::{io::BufReader, AsyncBufRead, AsyncRead, AsyncWrite, AsyncBufReadExt};

// /// Stream represent a IO stream.
// #[derive(Debug)]
// pub struct AsyncStream<S: Read> {
//     inner: BufReader<Async<BufferedReader<S>>>,
// }

// impl<S: AsRawFd + Read> AsyncStream<S> {
//     /// The function returns a new Stream from a file.
//     pub fn new(stream: S) -> io::Result<Self> {
//         let buffered_stream = BufferedReader::new(stream);
//         let async_stream = Async::new(buffered_stream)?;
//         let stream = BufReader::new(async_stream);
//         Ok(Self { inner: stream })
//     }
// }

// impl<S: Read> AsyncStream<S> {
//     pub fn keep_in_buffer(&mut self, v: &[u8]) {
//         self.inner.get_mut().get_mut().buffer.extend(v);
//     }

//     pub fn get_mut(&mut self) -> &mut S {
//         &mut self.inner.get_mut().get_mut().inner
//     }

//     pub fn get_available(&mut self) -> &[u8] {
//         &self.inner.get_ref().get_mut().buffer
//     }

//     pub fn consume_available(&mut self, n: usize) {
//         self.inner.get_mut().get_mut().buffer.drain(..n);
//     }
// }

// impl<S: Read> AsyncStream<S> {
//     pub fn flush_in_buffer(&mut self) {
//         // Because we have 2 buffered streams there might appear inconsistancy
//         // in read operations and the data which was via `keep_in_buffer` function.
//         //
//         // To eliminate it we move BufReader buffer to our buffer.
//         let b = self.inner.buffer().to_vec();
//         self.inner.consume(b.len());
//         self.keep_in_buffer(&b);
//     }
// }

// impl<S: Read + Write> AsyncWrite for AsyncStream<S> {
//     fn poll_write(
//         mut self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &[u8],
//     ) -> Poll<io::Result<usize>> {
//         Pin::new(self.get_mut()).poll_write(cx, buf)
//     }

//     fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
//         Pin::new(self.get_mut()).poll_flush(cx)
//     }

//     fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
//         Pin::new(self.get_mut()).poll_close(cx)
//     }

//     fn poll_write_vectored(
//         mut self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         bufs: &[io::IoSlice<'_>],
//     ) -> Poll<io::Result<usize>> {
//         Pin::new(self.get_mut()).poll_write_vectored(cx, bufs)
//     }
// }

// impl<S: Read> AsyncRead for AsyncStream<S> {
//     fn poll_read(
//         mut self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &mut [u8],
//     ) -> Poll<io::Result<usize>> {
//         Pin::new(&mut self.inner).poll_read(cx, buf)
//     }
// }

// impl<S: Read> AsyncBufRead for AsyncStream<S> {
//     fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
//         Pin::new(&mut self.inner).poll_fill_buf(cx)
//     }

//     fn consume(self: Pin<&mut Self>, amt: usize) {
//         Pin::new(&mut self.inner).consume(amt)
//     }
// }

// #[derive(Debug)]
// struct BufferedReader<R> {
//     inner: R,
//     buffer: Vec<u8>,
// }

// impl<R> BufferedReader<R> {
//     fn new(reader: R) -> Self {
//         Self {
//             inner: reader,
//             buffer: Vec::new(),
//         }
//     }
// }

// impl<R: Read> Read for BufferedReader<R> {
//     fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
//         if self.buffer.is_empty() {
//             self.inner.read(buf)
//         } else {
//             let n = buf.write(&self.buffer)?;
//             self.buffer.drain(..n);
//             Ok(n)
//         }
//     }
// }

// impl<R: AsRawFd> AsRawFd for BufferedReader<R> {
//     fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
//         self.inner.as_raw_fd()
//     }
// }
