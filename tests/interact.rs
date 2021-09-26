use std::{
    io::{self, Cursor, Read},
    time::{Duration, Instant},
};

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[ignore = "It requires manual interaction; Or it's necessary to redirect an stdin of current process"]
#[test]
fn interact_callback() {
    let mut session = expectrl::spawn("cat").unwrap();

    let opts = expectrl::interact::InteractOptions::terminal()
        .unwrap()
        .on_input("123", |session| {
            session.send_line("Hello World")?;
            Ok(())
        });

    opts.interact(&mut session).unwrap();
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn interact_stream_redirection() {
    let commands = "Hello World\nIt works :)\n";

    let reader = ReaderWithDelayEof::new(commands, Duration::from_secs(4));
    let mut writer = io::Cursor::new(vec![0; 1024]);

    let mut session = expectrl::spawn("cat").unwrap();
    let opts = expectrl::interact::InteractOptions::streamed(reader, &mut writer).unwrap();

    opts.interact(&mut session).unwrap();

    let buffer = String::from_utf8_lossy(writer.get_ref());
    let buffer = buffer.trim_end_matches(char::from(0));

    assert_eq!(buffer, "Hello World\r\nIt works :)\r\n");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn interact_stream_redirection() {
    futures_lite::future::block_on(async {
        let commands = "Hello World\nIt works :)\n";

        let reader = ReaderWithDelayEof::new(commands, Duration::from_secs(4));
        let mut writer = io::Cursor::new(vec![0; 1024]);

        let mut session = expectrl::spawn("cat").unwrap();
        let opts = expectrl::interact::InteractOptions::streamed(reader, &mut writer).unwrap();

        opts.interact(&mut session).await.unwrap();

        let buffer = String::from_utf8_lossy(writer.get_ref());
        let buffer = buffer.trim_end_matches(char::from(0));

        assert_eq!(buffer, "Hello World\r\nIt works :)\r\n");
    });
}

struct ReaderWithDelayEof<T> {
    inner: Cursor<T>,
    fire_timeout: Duration,
    now: Instant,
}

impl<T> ReaderWithDelayEof<T>
where
    T: AsRef<[u8]>,
{
    fn new(buf: T, timeout: Duration) -> Self {
        Self {
            inner: Cursor::new(buf),
            now: Instant::now(),
            fire_timeout: timeout,
        }
    }
}

impl<T> Read for ReaderWithDelayEof<T>
where
    T: AsRef<[u8]>,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n == 0 && self.now.elapsed() < self.fire_timeout {
            Err(io::Error::new(io::ErrorKind::WouldBlock, ""))
        } else {
            Ok(n)
        }
    }
}

#[cfg(feature = "async")]
impl<T> futures_lite::AsyncRead for ReaderWithDelayEof<T>
where
    T: AsRef<[u8]> + Unpin,
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
        let result = self.get_mut().read(buf);
        std::task::Poll::Ready(result)
    }
}
