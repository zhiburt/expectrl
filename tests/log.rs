use std::{
    io::{self, prelude::*, Cursor},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[cfg(feature = "async")]
use futures_lite::AsyncReadExt;

use expectrl::spawn;

#[test]
fn log() {
    let writer = StubWriter::default();
    let mut session = spawn("cat").unwrap().with_log(writer.clone()).unwrap();

    #[cfg(feature = "async")]
    futures_lite::future::block_on(async {
        session.send_line("Hello World").await.unwrap();

        // give some time to cat
        // since sometimes we doesn't keep up to read whole string
        thread::sleep(Duration::from_millis(300));

        let mut buf = vec![0; 1024];
        let _ = session.read(&mut buf).await.unwrap();
    });

    #[cfg(not(feature = "async"))]
    {
        session.send_line("Hello World").unwrap();

        // give some time to cat
        // since sometimes we doesn't keep up to read whole string
        thread::sleep(Duration::from_millis(300));

        let mut buf = vec![0; 1024];
        let _ = session.read(&mut buf).unwrap();
    }

    let bytes = writer.inner.lock().unwrap();
    assert_eq!(
        String::from_utf8_lossy(bytes.get_ref()),
        "write: \"Hello World\\n\"\nread: \"Hello World\\r\\n\"\n"
    )
}

#[test]
fn log_read_line() {
    let writer = StubWriter::default();
    let mut session = spawn("cat").unwrap().with_log(writer.clone()).unwrap();

    #[cfg(feature = "async")]
    futures_lite::future::block_on(async {
        session.send_line("Hello World").await.unwrap();

        let mut buf = String::new();
        let _ = session.read_line(&mut buf).await.unwrap();
        assert_eq!(buf, "Hello World\r\n");
    });

    #[cfg(not(feature = "async"))]
    {
        session.send_line("Hello World").unwrap();

        let mut buf = String::new();
        let _ = session.read_line(&mut buf).unwrap();
        assert_eq!(buf, "Hello World\r\n");
    }

    let bytes = writer.inner.lock().unwrap();
    assert_eq!(
        String::from_utf8_lossy(bytes.get_ref()),
        "write: \"Hello World\\n\"\nread: \"Hello World\\r\\n\"\n"
    )
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