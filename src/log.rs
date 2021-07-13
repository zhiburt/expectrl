#![cfg(any(feature = "log", feature = "async_log"))]
//! A wrapper of Session to log a read/write operations

use crate::{error::Error, session::Session};
use std::{
    io::{self, Write},
    ops::{Deref, DerefMut},
    process::Command,
};

/// A logging wrapper of session
pub struct SessionWithLog {
    inner: Session,
    writer: Option<Box<dyn Write>>,
}
impl SessionWithLog {
    pub fn spawn(cmd: &str) -> Result<Self, Error> {
        let session = Session::spawn(cmd)?;
        Ok(Self {
            inner: session,
            writer: None,
        })
    }

    pub fn spawn_cmd(cmd: Command) -> Result<Self, Error> {
        let session = Session::spawn_cmd(cmd)?;
        Ok(Self {
            inner: session,
            writer: None,
        })
    }

    pub fn set_writer<W: Write + 'static>(&mut self, w: W) {
        self.writer = Some(Box::new(w));
    }

    fn log(&mut self, target: &str, data: &[u8]) {
        if let Some(writer) = self.writer.as_mut() {
            let _ = match std::str::from_utf8(data) {
                Ok(s) => writeln!(writer, "{} {:?}", target, s),
                Err(..) => writeln!(writer, "{} (bytes) {:?}", target, data),
            };
        }
    }
}

#[cfg(feature = "log")]
impl SessionWithLog {
    pub fn send<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.log("send", s.as_ref().as_bytes());
        self.inner.send(s)
    }

    pub fn send_line<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.log("send_line", s.as_ref().as_bytes());
        self.inner.send_line(s)
    }
}

#[cfg(feature = "async_log")]
impl SessionWithLog {
    pub async fn send<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.log("send", s.as_ref().as_bytes());
        self.inner.send(s).await
    }

    pub async fn send_line<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.log("send_line", s.as_ref().as_bytes());
        self.inner.send_line(s).await
    }
}

impl Deref for SessionWithLog {
    type Target = Session;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for SessionWithLog {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(feature = "log")]
impl std::io::Write for SessionWithLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.log("write", buf);
        self.deref_mut().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.deref_mut().flush()
    }
}

#[cfg(feature = "log")]
impl std::io::Read for SessionWithLog {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let result = self.deref_mut().read(buf);
        if let Ok(n) = result {
            self.log("read", &buf[..n]);
        }

        result
    }
}

#[cfg(feature = "async_log")]
impl futures_lite::io::AsyncRead for SessionWithLog {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let result = futures_lite::io::AsyncRead::poll_read(
            std::pin::Pin::new(self.inner.deref_mut().deref_mut()), // haven't foudn any better way
            cx,
            buf,
        );

        if let std::task::Poll::Ready(Ok(n)) = result {
            self.log("read", &buf[..n]);
        }

        result
    }
}

#[cfg(feature = "async_log")]
impl futures_lite::io::AsyncWrite for SessionWithLog {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.log("write", &buf);
        std::pin::Pin::new(self.inner.deref_mut().deref_mut()).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(self.inner.deref_mut().deref_mut()).poll_flush(cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(self.inner.deref_mut().deref_mut()).poll_flush(cx)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{
        io::Cursor,
        sync::{Arc, Mutex},
    };

    #[cfg(feature = "sync")]
    #[cfg(feature = "log")]
    #[test]
    fn log() {
        use std::io::Read;

        let mut session = SessionWithLog::spawn("cat").unwrap();
        let writer = StubWriter::default();
        session.set_writer(writer.clone());
        session.send_line("Hello World").unwrap();

        let mut buf = vec![0; 1024];
        let _ = session.read(&mut buf).unwrap();

        let bytes = writer.inner.lock().unwrap();
        assert_eq!(
            String::from_utf8_lossy(bytes.get_ref()),
            "send_line \"Hello World\"\n\
             read \"Hello World\\r\\n\"\n"
        )
    }

    #[cfg(feature = "async_log")]
    #[cfg(feature = "async")]
    #[test]
    fn log() {
        use futures_lite::AsyncReadExt;

        futures_lite::future::block_on(async {
            let mut session = SessionWithLog::spawn("cat").unwrap();
            let writer = StubWriter::default();
            session.set_writer(writer.clone());
            session.send_line("Hello World").await.unwrap();

            let mut buf = vec![0; 1024];
            let _ = session.read(&mut buf).await.unwrap();

            let bytes = writer.inner.lock().unwrap();
            assert_eq!(
                String::from_utf8_lossy(bytes.get_ref()),
                "send_line \"Hello World\"\n\
             read \"Hello World\\r\\n\"\n"
            )
        })
    }

    #[cfg(feature = "async_log")]
    #[cfg(feature = "async")]
    #[test]
    fn deref() {
        use futures_lite::AsyncReadExt;

        futures_lite::future::block_on(async {
            let mut session = crate::Session::spawn("cat").unwrap();
            let writer = StubWriter::default();
            session.set_writer(writer.clone());
            session.send_line("Hello World").await.unwrap();

            let mut buf = vec![0; 1024];
            let _ = session.read(&mut buf).await.unwrap();

            let bytes = writer.inner.lock().unwrap();
            assert_eq!(
                String::from_utf8_lossy(bytes.get_ref()),
                "send_line \"Hello World\"\n\
             read \"Hello World\\r\\n\"\n"
            )
        })
    }

    #[cfg(feature = "async_log")]
    #[cfg(feature = "async")]
    #[test]
    fn log_bash() {
        use futures_lite::AsyncBufReadExt;

        futures_lite::future::block_on(async {
            let mut bash = crate::repl::spawn_bash().await.unwrap();
            let writer = StubWriter::default();
            bash.set_writer(writer.clone());
            bash.send_line("echo Hello World").await.unwrap();

            let mut buf = String::new();
            let _ = bash.read_line(&mut buf).await.unwrap();

            let bytes = writer.inner.lock().unwrap();
            assert_eq!(
                String::from_utf8_lossy(bytes.get_ref()),
                "send_line \"echo Hello World\"\n"
            )
        })
    }

    #[derive(Debug, Clone, Default)]
    struct StubWriter {
        inner: Arc<Mutex<Cursor<Vec<u8>>>>,
    }

    impl Write for StubWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.inner.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.inner.lock().unwrap().flush()
        }
    }
}
