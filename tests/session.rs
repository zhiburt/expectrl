use expectrl::{spawn, Session};
use std::{thread, time::Duration};

#[cfg(feature = "async")]
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(not(feature = "async"))]
#[cfg(not(windows))]
use std::io::{Read, Write};

#[cfg(windows)]
use std::io::BufRead;

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn send() {
    let mut session = spawn("cat").unwrap();
    session.send("Hello World").unwrap();

    thread::sleep(Duration::from_millis(300));
    session.write_all(&[3]).unwrap(); // Ctrl+C
    session.flush().unwrap();

    let mut buf = String::new();
    session.read_to_string(&mut buf).unwrap();

    // cat doesn't printed anything
    assert_eq!(buf, "");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn send() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send("Hello World").await.unwrap();

        thread::sleep(Duration::from_millis(300));
        session.write_all(&[3]).await.unwrap(); // Ctrl+C
        session.flush().await.unwrap();

        let mut buf = String::new();
        session.read_to_string(&mut buf).await.unwrap();

        // cat doesn't printed anything
        assert_eq!(buf, "");
    })
}

#[cfg(windows)]
#[test]
fn send() {
    let mut session = spawn("powershell -C type").unwrap();
    #[cfg(not(feature = "async"))]
    {
        session.write(b"Hello World").unwrap();
        thread::sleep(Duration::from_millis(300));
        session.expect("Hello World").unwrap();
    }
    #[cfg(feature = "async")]
    {
        futures_lite::future::block_on(async {
            session.write(b"Hello World").await.unwrap();
            thread::sleep(Duration::from_millis(300));
            session.expect("Hello World").await.unwrap();
        })
    }
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn send_multiline() {
    let mut session = spawn("cat").unwrap();
    session.send("Hello World\n").unwrap();

    let m = session.expect('\n').unwrap();
    let buf = String::from_utf8_lossy(m.before());

    assert_eq!(buf, "Hello World\r");

    session.exit(true).unwrap();
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn send_multiline() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send("Hello World\n").await.unwrap();

        thread::sleep(Duration::from_millis(300));
        session.write_all(&[3]).await.unwrap(); // Ctrl+C
        session.flush().await.unwrap();

        let mut buf = String::new();
        session.read_to_string(&mut buf).await.unwrap();

        // cat repeats a send line after <enter> is presend
        // <enter> is basically a new line
        assert_eq!(buf, "Hello World\r\n");
    })
}

#[cfg(windows)]
#[test]
fn send_multiline() {
    let mut session = spawn("powershell -C type").unwrap();
    #[cfg(not(feature = "async"))]
    {
        session.send("Hello World\r\n").unwrap();

        thread::sleep(Duration::from_millis(300));

        let buf = session.lines().nth(2).unwrap().unwrap();

        if !buf.contains("Hello World") {
            panic!(
                "Expected to get {:?} in the output, but got {:?}",
                "Hello World", buf
            );
        }
    }
    #[cfg(feature = "async")]
    {
        use futures_lite::{AsyncBufReadExt, StreamExt};

        futures_lite::future::block_on(async {
            session.send("Hello World\r\n").await.unwrap();

            thread::sleep(Duration::from_millis(300));

            let buf = session.lines().nth(2).await.unwrap().unwrap();

            if !buf.contains("Hello World") {
                panic!(
                    "Expected to get {:?} in the output, but got {:?}",
                    "Hello World", buf
                );
            }
        })
    }
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn send_line() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();

    let m = session.expect('\n').unwrap();
    let buf = String::from_utf8_lossy(m.before());

    assert_eq!(buf, "Hello World\r");

    session.exit(true).unwrap();
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn send_line() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();

        thread::sleep(Duration::from_millis(300));
        session.exit(true).unwrap();
        thread::sleep(Duration::from_millis(300));

        let mut buf = String::new();
        session.read_to_string(&mut buf).await.unwrap();

        // cat repeats a send line after <enter> is presend
        // <enter> is basically a new line
        //
        // NOTE: in debug mode though it equals 'Hello World\r\n'
        // : stty -a are the same
        assert_eq!(buf, "Hello World\r\n");
    })
}

#[cfg(windows)]
#[test]
fn send_line() {
    let mut session = spawn("powershell -C type").unwrap();
    #[cfg(not(feature = "async"))]
    {
        session.send_line("Hello World").unwrap();

        thread::sleep(Duration::from_millis(300));

        let buf = session.lines().nth(2).unwrap().unwrap();

        if !buf.contains("Hello World") {
            panic!(
                "Expected to get {:?} in the output, but got {:?}",
                "Hello World", buf
            );
        }
    }
    #[cfg(feature = "async")]
    {
        use futures_lite::{AsyncBufReadExt, StreamExt};

        futures_lite::future::block_on(async {
            session.send_line("Hello World").await.unwrap();

            thread::sleep(Duration::from_millis(300));

            let buf = session.lines().nth(2).await.unwrap().unwrap();

            if !buf.contains("Hello World") {
                panic!(
                    "Expected to get {:?} in the output, but got {:?}",
                    "Hello World", buf
                );
            }
        })
    }
}

#[test]
fn test_spawn_no_command() {
    #[cfg(unix)]
    assert!(spawn("").is_err());
    #[cfg(windows)]
    assert!(spawn("").is_ok());
}

#[test]
#[ignore = "it's a compile time check"]
fn test_session_as_writer() {
    #[cfg(not(feature = "async"))]
    {
        let _: Box<dyn std::io::Write> = Box::new(spawn("ls").unwrap());
        let _: Box<dyn std::io::Read> = Box::new(spawn("ls").unwrap());
        let _: Box<dyn std::io::BufRead> = Box::new(spawn("ls").unwrap());

        fn _io_copy(mut session: Session) {
            let _ = std::io::copy(&mut std::io::empty(), &mut session).unwrap();
        }
    }
    #[cfg(feature = "async")]
    {
        let _: Box<dyn futures_lite::AsyncWrite> =
            Box::new(spawn("ls").unwrap()) as Box<dyn futures_lite::AsyncWrite>;
        let _: Box<dyn futures_lite::AsyncRead> =
            Box::new(spawn("ls").unwrap()) as Box<dyn futures_lite::AsyncRead>;
        let _: Box<dyn futures_lite::AsyncBufRead> =
            Box::new(spawn("ls").unwrap()) as Box<dyn futures_lite::AsyncBufRead>;

        async fn _io_copy(mut session: Session) {
            futures_lite::io::copy(futures_lite::io::empty(), &mut session)
                .await
                .unwrap();
        }
    }
}
