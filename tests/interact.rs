use std::{
    io::{self, Cursor, Read, Write},
    time::{Duration, Instant},
};

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[ignore = "It requires manual interaction; Or it's necessary to redirect an stdin of current process"]
#[test]
fn interact_callback() {
    let mut session = expectrl::spawn("cat").unwrap();

    let mut opts = expectrl::interact::InteractOptions::terminal()
        .unwrap()
        .on_input("123", |mut ctx| {
            ctx.session().send_line("Hello World")?;
            Ok(())
        })
        .on_output(b'\n', |_, f| {
            let line = f.before();
            println!("Line in output {:?}", String::from_utf8_lossy(line));
            Ok(())
        });

    opts.interact(&mut session).unwrap();
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn interact_callbacks_with_stream_redirection() {
    let commands = vec![
        "NO_MATCHED\n".to_string(),
        "QWE\n".to_string(),
        "QW123\n".to_string(),
        "NO_MATCHED_2\n".to_string(),
    ];

    let reader = ListReaderWithDelayedEof::new(commands, Duration::from_secs(3));
    let mut writer = io::Cursor::new(vec![0; 2048]);

    let mut session = expectrl::spawn("cat").unwrap();
    let mut opts = expectrl::interact::InteractOptions::streamed(reader, &mut writer)
        .unwrap()
        .on_input("QWE", |mut ctx| {
            ctx.session().send_line("Hello World")?;
            Ok(())
        });

    opts.interact(&mut session).unwrap();

    drop(opts);

    let buffer = String::from_utf8_lossy(writer.get_ref());
    let buffer = buffer.trim_end_matches(char::from(0));

    assert_eq!(
        buffer,
        "NO_MATCHED\r\nHello World\r\n\r\nQW123\r\nNO_MATCHED_2\r\n"
    );
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn interact_callbacks_called_after_exit() {
    use expectrl::WaitStatus;

    let mut session = expectrl::spawn("echo 'Hello World'").unwrap();
    let mut opts = expectrl::interact::InteractOptions::terminal()
        .unwrap()
        .state(0)
        .on_output("World", |mut ctx, _| {
            *ctx.state() += 1;
            Ok(())
        });

    assert_eq!(
        session.wait().unwrap(),
        WaitStatus::Exited(session.pid(), 0)
    );

    assert_eq!(
        opts.interact(&mut session).unwrap_err().to_string(),
        "Nix error ECHILD: No child processes"
    );

    #[cfg(target_os = "linux")]
    assert_eq!(*opts.get_state(), 1);

    #[cfg(not(target_os = "linux"))]
    assert_eq!(*opts.get_state(), 0);
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn interact_filters() {
    let commands = "1009\nNO\n";

    let reader = ReaderWithDelayEof::new(commands, Duration::from_secs(4));
    let mut writer = io::Cursor::new(vec![0; 2048]);

    let mut session = expectrl::spawn("cat").unwrap();
    let mut opts = expectrl::interact::InteractOptions::streamed(reader, &mut writer)
        .unwrap()
        .input_filter(|buf| {
            // ignore 0 chars
            let v = buf.iter().filter(|&&b| b != b'0').copied().collect();
            Ok(v)
        })
        .output_filter(|buf| {
            // Make NO -> YES
            let v = buf
                .chunks(2)
                .map(|s| match s {
                    &[b'N', b'O'] => &[b'Y', b'E', b'S'],
                    other => other,
                })
                .flatten()
                .copied()
                .collect();
            Ok(v)
        });

    opts.interact(&mut session).unwrap();

    drop(opts);

    let buffer = String::from_utf8_lossy(writer.get_ref());
    let buffer = buffer.trim_end_matches(char::from(0));

    assert_eq!(buffer, "19\r\nYES\r\n");
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn interact_context() {
    let commands = vec![
        "QWE\n".to_string(),
        "QWE\n".to_string(),
        "QWE\n".to_string(),
    ];

    let reader = ListReaderWithDelayedEof::new(commands, Duration::from_secs(3));
    let mut writer = io::Cursor::new(vec![0; 2048]);

    let mut session = expectrl::spawn("cat").unwrap();
    let mut opts = expectrl::interact::InteractOptions::streamed(reader, &mut writer)
        .unwrap()
        .state((0, 0))
        .on_input("QWE\n", |mut ctx| {
            let state = ctx.state();
            state.0 += 1;
            ctx.session().send_line("123")?;
            Ok(())
        })
        .on_output(expectrl::NBytes(1), |mut ctx, _| {
            let state = ctx.state();
            state.1 += 1;
            Ok(())
        });

    opts.interact(&mut session).unwrap();

    assert_eq!(opts.get_state().0, 3);
    assert_eq!(opts.get_state().1, 15);

    drop(opts);

    let buffer = String::from_utf8_lossy(writer.get_ref());
    let buffer = buffer.trim_end_matches(char::from(0));
    assert_eq!(buffer, "123\r\n123\r\n123\r\n");
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn interact_on_output_not_matched() {
    // Stops interact mode after 123 being read.
    // Which may cause it to stay buffered in session.
    // Verify this buffer was cleaned and 123 won't be accessed then.

    let commands = vec![
        "QWE\n".to_string(),
        "123\n".to_string(),
        String::from_utf8_lossy(&[29]).to_string(),
        "WWW\n".to_string(),
    ];

    let reader = ListReaderWithDelayedEof::new(commands, Duration::from_secs(3));
    let mut writer = io::Cursor::new(vec![0; 2048]);

    let mut session = expectrl::spawn("cat").unwrap();
    let mut opts = expectrl::interact::InteractOptions::streamed(reader, &mut writer)
        .unwrap()
        .state((0, 0))
        .on_input("WWW\n", |mut ctx| {
            ctx.state().1 += 1;
            Ok(())
        })
        .on_input("QWE\n", |mut ctx| {
            ctx.state().0 += 1;
            Ok(())
        })
        .on_output("NOT_FOUND_IN_THE_OUTPUT", |_, _| Ok(()))
        .on_idle(|_| {
            std::thread::sleep(Duration::from_millis(1000));
            Ok(())
        });

    opts.interact(&mut session).unwrap();

    assert_eq!(opts.get_state().0, 1);
    assert_eq!(opts.get_state().1, 0);

    drop(opts);

    let buffer = String::from_utf8_lossy(writer.get_ref());
    let buffer = buffer.trim_end_matches(char::from(0));
    assert_eq!(buffer, "123\r\n");

    session.send_line("WWW").unwrap();

    let m = session.expect("WWW\r\n").unwrap();
    assert_ne!(m.before(), b"123\r\n");
    assert_eq!(m.before(), b"");
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn interact_stream_redirection() {
    let commands = "Hello World\nIt works :)\n";

    let reader = ReaderWithDelayEof::new(commands, Duration::from_secs(4));
    let mut writer = io::Cursor::new(vec![0; 1024]);

    let mut session = expectrl::spawn("cat").unwrap();
    let mut opts = expectrl::interact::InteractOptions::streamed(reader, &mut writer).unwrap();

    opts.interact(&mut session).unwrap();

    drop(opts);

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

struct ListReaderWithDelayedEof {
    lines: Vec<String>,
    eof_timeout: Duration,
    now: Option<Instant>,
}

impl ListReaderWithDelayedEof {
    fn new(lines: Vec<String>, eof_timeout: Duration) -> Self {
        Self {
            lines,
            eof_timeout,
            now: None,
        }
    }
}

impl Read for ListReaderWithDelayedEof {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        if self.now.is_none() {
            self.now = Some(Instant::now());
        }

        if !self.lines.is_empty() {
            let line = self.lines.remove(0);
            buf.write_all(line.as_bytes())?;
            Ok(line.as_bytes().len())
        } else if self.now.unwrap().elapsed() < self.eof_timeout {
            Err(io::Error::new(io::ErrorKind::WouldBlock, ""))
        } else {
            Ok(0)
        }
    }
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
