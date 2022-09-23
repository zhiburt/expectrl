use std::{
    io::{self, sink, Cursor, Read, Write},
    time::{Duration, Instant},
};

use expectrl::{interact::actions::lookup::Lookup, spawn, stream::stdin::Stdin, NBytes};

#[cfg(unix)]
use expectrl::WaitStatus;

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[ignore = "It requires manual interaction; Or it's necessary to redirect an stdin of current process"]
#[test]
fn interact_callback() {
    let mut input_handle = Lookup::new();
    let mut output_handle = Lookup::new();

    let mut session = spawn("cat").unwrap();

    let mut stdin = Stdin::open().unwrap();

    session
        .interact(&mut stdin, sink())
        .on_input(|ctx| {
            if input_handle.on(ctx.buf, ctx.eof, "213")?.is_some() {
                ctx.session.send_line("Hello World")?;
            }

            Ok(())
        })
        .on_output(|ctx| {
            if let Some(m) = output_handle.on(ctx.buf, ctx.eof, b'\n')? {
                let line = m.before();
                println!("Line in output {:?}", String::from_utf8_lossy(line));
            }

            Ok(())
        })
        .spawn()
        .unwrap();

    stdin.close().unwrap();
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn interact_output_callback() {
    let mut session = expectrl::spawn("sleep 1 && echo 'Hello World'").unwrap();

    let mut stdin = Stdin::open().unwrap();
    let stdout = std::io::sink();

    let mut state = 0;

    let mut lookup = Lookup::new();
    session
        .interact(&mut stdin, stdout)
        .set_state(&mut state)
        .on_output(|ctx| {
            if lookup.on(ctx.buf, ctx.eof, "World")?.is_some() {
                **ctx.state += 1;
            }

            Ok(())
        })
        .spawn()
        .unwrap();

    stdin.close().unwrap();

    // fixme: sometimes it's 0
    //        I guess because the process gets down to fast.

    assert_eq!(state, 1);
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn interact_callbacks_called_after_exit() {
    let mut session = expectrl::spawn("echo 'Hello World'").unwrap();

    assert_eq!(
        session.wait().unwrap(),
        WaitStatus::Exited(session.pid(), 0)
    );

    let mut stdin = Stdin::open().unwrap();
    let stdout = std::io::sink();

    let mut state = 0;

    let mut lookup = Lookup::new();
    session
        .interact(&mut stdin, stdout)
        .set_state(&mut state)
        .on_output(|ctx| {
            if lookup.on(ctx.buf, ctx.eof, "World")?.is_some() {
                **ctx.state += 1;
            }

            Ok(())
        })
        .spawn()
        .unwrap();

    stdin.close().unwrap();

    assert_eq!(state, 0);
}

#[cfg(unix)]
#[cfg(not(any(feature = "async", feature = "polling")))]
#[test]
fn interact_callbacks_with_stream_redirection() {
    let output_lines = vec![
        "NO_MATCHED\n".to_string(),
        "QWE\n".to_string(),
        "QW123\n".to_string(),
        "NO_MATCHED_2\n".to_string(),
    ];

    let reader = ListReaderWithDelayedEof::new(output_lines, Duration::from_secs(2));
    let mut writer = io::Cursor::new(vec![0; 2048]);

    let mut session = spawn("cat").unwrap();

    let mut input_handle = Lookup::new();
    session
        .interact(reader, &mut writer)
        .on_input(|ctx| {
            if input_handle.on(ctx.buf, ctx.eof, "QWE")?.is_some() {
                ctx.session.send_line("Hello World")?;
            };

            Ok(())
        })
        .spawn()
        .unwrap();

    let buffer = String::from_utf8_lossy(writer.get_ref());
    assert!(buffer.contains("Hello World"), "{:?}", buffer);
}

#[cfg(unix)]
#[cfg(not(any(feature = "async", feature = "polling")))]
#[test]
fn interact_filters() {
    let reader = ReaderWithDelayEof::new("1009\nNO\n", Duration::from_secs(4));
    let mut writer = io::Cursor::new(vec![0; 2048]);

    let mut session = spawn("cat").unwrap();
    session
        .interact(reader, &mut writer)
        .input_filter(|buf| {
            // ignore 0 chars
            let v = buf.iter().filter(|&&b| b != b'0').copied().collect();
            Ok(v)
        })
        .output_filter(|buf| {
            // Make NO -> YES
            let v = buf
                .chunks(2)
                .flat_map(|s| match s {
                    &[b'N', b'O'] => &[b'Y', b'E', b'S'],
                    other => other,
                })
                .copied()
                .collect();
            Ok(v)
        })
        .spawn()
        .unwrap();

    let buffer = String::from_utf8_lossy(writer.get_ref());
    let buffer = buffer.trim_end_matches(char::from(0));

    // fixme: somehow the output is duplicated which is wrong.
    assert_eq!(buffer, "19\r\nYES\r\n19\r\nYES\r\n");
}

#[cfg(all(unix, not(any(feature = "async", feature = "polling"))))]
#[test]
fn interact_context() {
    let mut session = spawn("cat").unwrap();

    let reader = ListReaderWithDelayedEof::new(
        vec![
            "QWE\n".into(),
            "QWE\n".into(),
            "QWE\n".into(),
            "QWE\n".into(),
        ],
        Duration::from_secs(2),
    );
    let mut writer = io::Cursor::new(vec![0; 2048]);

    let mut input_data = Lookup::new();
    let mut output_data = Lookup::new();

    let state = session
        .interact(reader, &mut writer)
        .set_state((0, 0))
        .on_input(|ctx| {
            if input_data.on(ctx.buf, ctx.eof, "QWE\n")?.is_some() {
                ctx.state.0 += 1;
                ctx.session.send_line("123")?;
            }

            Ok(())
        })
        .on_output(|ctx| {
            if output_data.on(ctx.buf, ctx.eof, NBytes(1))?.is_some() {
                ctx.state.1 += 1;
                output_data.clear();
            }

            Ok(())
        })
        .spawn()
        .unwrap();

    assert_eq!(state.0, 4);
    assert!(state.1 > 0, "{:?}", state.1);

    let buffer = String::from_utf8_lossy(writer.get_ref());
    assert!(buffer.contains("123"), "{:?}", buffer);
}

#[cfg(all(unix, not(any(feature = "async", feature = "polling"))))]
#[test]
fn interact_on_output_not_matched() {
    // Stops interact mode after 123 being read.
    // Which may cause it to stay buffered in session.
    // Verify this buffer was cleaned and 123 won't be accessed then.

    let reader = ListReaderWithDelayedEof::new(
        vec![
            "QWE\n".to_string(),
            "123\n".to_string(),
            String::from_utf8_lossy(&[29]).to_string(),
            "WWW\n".to_string(),
        ],
        Duration::from_secs(2),
    );
    let mut writer = io::Cursor::new(vec![0; 2048]);

    let mut input = Lookup::new();

    let mut session = spawn("cat").unwrap();
    let state = session
        .interact(reader, &mut writer)
        .set_state((0, 0))
        .on_input(|ctx| {
            if input.on(ctx.buf, ctx.eof, "QWE\n")?.is_some() {
                ctx.state.0 += 1;
            }

            if input.on(ctx.buf, ctx.eof, "WWW\n")?.is_some() {
                ctx.state.1 += 1;
            }

            Ok(())
        })
        .on_output(|_ctx| Ok(()))
        .on_idle(|_ctx| {
            std::thread::sleep(Duration::from_millis(500));
            Ok(())
        })
        .spawn()
        .unwrap();

    assert_eq!(state.0, 2);
    assert_eq!(state.1, 0);

    let buffer = String::from_utf8_lossy(writer.get_ref());
    let buffer = buffer.trim_end_matches(char::from(0));
    assert_eq!(buffer, "QWE\r\nQWE\r\n123\r\n123\r\n");

    session.send_line("WWW").unwrap();

    let m = session.expect("WWW\r\n").unwrap();
    assert_ne!(m.before(), b"123\r\n");
    assert_eq!(m.before(), b"");
}

// #[cfg(unix)]
// #[cfg(not(feature = "polling"))]
// #[cfg(not(feature = "async"))]
// #[test]
// fn interact_stream_redirection() {
//     let commands = "Hello World\nIt works :)\n";

//     let mut reader = ReaderWithDelayEof::new(commands, Duration::from_secs(4));
//     let mut writer = io::Cursor::new(vec![0; 1024]);

//     let mut session = expectrl::spawn("cat").unwrap();
//     let mut opts = expectrl::interact::InteractOptions::default();

//     opts.interact(&mut session, &mut reader, &mut writer)
//         .unwrap();

//     drop(opts);

//     let buffer = String::from_utf8_lossy(writer.get_ref());
//     let buffer = buffer.trim_end_matches(char::from(0));

//     assert_eq!(buffer, "Hello World\r\nIt works :)\r\n");
// }

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn interact_stream_redirection() {
    futures_lite::future::block_on(async {
        let commands = "Hello World\nIt works :)\n";

        let reader = ReaderWithDelayEof::new(commands, Duration::from_secs(4));
        let mut writer = io::Cursor::new(vec![0; 1024]);

        let mut session = expectrl::spawn("cat").unwrap();

        session.interact(reader, &mut writer).spawn().await.unwrap();

        let buffer = String::from_utf8_lossy(writer.get_ref());
        let buffer = buffer.trim_end_matches(char::from(0));

        assert_eq!(
            buffer,
            "Hello World\r\nIt works :)\r\nHello World\r\nIt works :)\r\n"
        );
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

#[cfg(unix)]
impl std::os::unix::io::AsRawFd for ListReaderWithDelayedEof {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        0
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
