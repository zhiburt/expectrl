#![cfg(feature = "log")]

use std::{
    io::{self, Cursor, Read, Write},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use expectrl::spawn;

#[cfg(not(feature = "async"))]
#[test]
fn log() {
    let mut session = spawn("cat").unwrap();
    let writer = StubWriter::default();
    session.set_log(writer.clone());
    session.send_line("Hello World").unwrap();

    // give some time to cat
    // since sometimes we doesn't keep up to read whole string
    thread::sleep(Duration::from_millis(300));

    let mut buf = vec![0; 1024];
    let _ = session.read(&mut buf).unwrap();

    let bytes = writer.inner.lock().unwrap();
    assert_eq!(
        String::from_utf8_lossy(bytes.get_ref()),
        "send_line \"Hello World\"\nread \"Hello World\\r\\n\"\n"
    )
}

#[cfg(not(feature = "async"))]
#[cfg(feature = "log")]
#[test]
fn log_read_line() {
    use std::io::BufRead;

    let mut session = spawn("cat").unwrap();
    let writer = StubWriter::default();
    session.set_log(writer.clone());
    session.send_line("Hello World").unwrap();

    let mut buf = String::new();
    let _ = session.read_line(&mut buf).unwrap();
    assert_eq!(buf, "Hello World\r\n");

    let bytes = writer.inner.lock().unwrap();
    assert_eq!(
        String::from_utf8_lossy(bytes.get_ref()),
        "send_line \"Hello World\"\n\
             read \"Hello World\\r\\n\"\n"
    )
}

#[cfg(all(feature = "async", feature = "log"))]
#[test]
fn log() {
    use futures_lite::AsyncReadExt;

    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        let writer = StubWriter::default();
        session.set_log(writer.clone());
        session.send_line("Hello World").await.unwrap();

        // give some time to cat
        // since sometimes we doesn't keep up to read whole string
        thread::sleep(Duration::from_millis(300));

        let mut buf = vec![0; 1024];
        let _ = session.read(&mut buf).await.unwrap();

        let bytes = writer.inner.lock().unwrap();
        assert_eq!(
            String::from_utf8_lossy(bytes.get_ref()),
            "send_line \"Hello World\"\nread \"Hello World\\r\\n\"\n"
        )
    })
}

#[cfg(all(feature = "async", feature = "log"))]
#[test]
fn deref() {
    use futures_lite::AsyncReadExt;

    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        let writer = StubWriter::default();
        session.set_log(writer.clone());
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

#[cfg(all(feature = "async", feature = "log"))]
#[test]
fn log_bash() {
    futures_lite::future::block_on(async {
        let mut bash = expectrl::repl::spawn_bash().await.unwrap();
        let writer = StubWriter::default();
        bash.set_log(writer.clone());
        bash.send_line("echo Hello World").await.unwrap();

        let mut buf = String::new();
        let _ = bash.read_line(&mut buf).await.unwrap();

        let bytes = writer.inner.lock().unwrap();
        let s = String::from_utf8_lossy(bytes.get_ref());
        assert!(s.starts_with("send_line \"echo Hello World\""));
        // We use contains and not direct comparision because the actuall output depends on the shell.
        assert!(s.contains("read"));
    })
}

#[cfg(all(feature = "async", feature = "log"))]
#[test]
fn log_read_line() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        let writer = StubWriter::default();
        session.set_log(writer.clone());
        session.send_line("Hello World").await.unwrap();

        let mut buf = String::new();
        let _ = session.read_line(&mut buf).await.unwrap();
        assert_eq!(buf, "Hello World\r\n");

        let bytes = writer.inner.lock().unwrap();
        assert_eq!(
            String::from_utf8_lossy(bytes.get_ref()),
            "send_line \"Hello World\"\nread \"Hello World\\r\\n\"\n"
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
