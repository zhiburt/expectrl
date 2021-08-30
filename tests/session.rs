#![cfg(unix)]
use expectrl::{spawn, Eof, NBytes, Regex};
use std::{thread, time::Duration};

#[cfg(feature = "async")]
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(not(feature = "async"))]
use std::io::{Read, Write};

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

#[cfg(not(feature = "async"))]
#[test]
fn send_multiline() {
    let mut session = spawn("cat").unwrap();
    session.send("Hello World\n").unwrap();

    thread::sleep(Duration::from_millis(300));
    session.write_all(&[3]).unwrap(); // Ctrl+C
    session.flush().unwrap();

    let mut buf = String::new();
    session.read_to_string(&mut buf).unwrap();

    // cat repeats a send line after <enter> is presend
    // <enter> is basically a new line
    assert_eq!(buf, "Hello World\r\n");
}

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

#[cfg(not(feature = "async"))]
#[test]
fn send_line() {
    let mut session = spawn("cat").unwrap();
    let _ = session.send_line("Hello World").unwrap();

    thread::sleep(Duration::from_millis(300));
    session.exit(true).unwrap();
    thread::sleep(Duration::from_millis(300));

    let mut buf = String::new();
    session.read_to_string(&mut buf).unwrap();

    // cat repeats a send line after <enter> is presend
    // <enter> is basically a new line
    //
    // NOTE: in debug mode though it equals 'Hello World\r\n'
    // : stty -a are the same
    assert_eq!(buf, "Hello World\r\n");
}

#[cfg(feature = "async")]
#[test]
fn send_line() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        let _ = session.send_line("Hello World").await.unwrap();

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

#[cfg(not(feature = "async"))]
#[test]
fn expect_str() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    session.expect("Hello World").unwrap();
}

#[cfg(feature = "async")]
#[test]
fn expect_str() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        session.expect("Hello World").await.unwrap();
    })
}

#[cfg(not(feature = "async"))]
#[test]
fn expect_regex() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    let m = session.expect(Regex("lo.*")).unwrap();
    assert_eq!(m.before_match(), b"Hel");
    assert_eq!(m.found_match(), b"lo");
}

#[cfg(feature = "async")]
#[test]
fn expect_regex() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        let m = session.expect(Regex("lo.*")).await.unwrap();
        assert_eq!(m.before_match(), b"Hel");
        assert_eq!(m.found_match(), b"lo");
    })
}

#[cfg(not(feature = "async"))]
#[test]
fn expect_n_bytes() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    let m = session.expect(NBytes(3)).unwrap();
    assert_eq!(m.found_match(), b"Hel");
    assert_eq!(m.before_match(), b"");
}

#[cfg(feature = "async")]
#[test]
fn expect_n_bytes() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        let m = session.expect(NBytes(3)).await.unwrap();
        assert_eq!(m.found_match(), b"Hel");
        assert_eq!(m.before_match(), b"");
    })
}

#[cfg(not(feature = "async"))]
#[test]
fn expect_eof() {
    let mut session = spawn("echo 'Hello World'").unwrap();
    session.set_expect_timeout(None);
    let m = session.expect(Eof).unwrap();
    assert_eq!(m.found_match(), b"'Hello World'\r\n");
    assert_eq!(m.before_match(), b"");
}

#[cfg(feature = "async")]
#[test]
fn expect_eof() {
    futures_lite::future::block_on(async {
        let mut session = spawn("echo 'Hello World'").unwrap();
        session.set_expect_timeout(None);
        let m = session.expect(Eof).await.unwrap();
        assert_eq!(m.found_match(), b"'Hello World'\r\n");
        assert_eq!(m.before_match(), b"");
    })
}

#[cfg(not(feature = "async"))]
#[test]
fn read_after_expect_str() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    session.expect("Hello").unwrap();

    let mut buf = [0; 6];
    session.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b" World");
}

#[cfg(feature = "async")]
#[test]
fn read_after_expect_str() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        session.expect("Hello").await.unwrap();

        let mut buf = [0; 6];
        session.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b" World");
    })
}

#[cfg(not(feature = "async"))]
#[test]
fn expect_eof_timeout() {
    let mut p = spawn("sleep 3").expect("cannot run sleep 3");
    p.set_expect_timeout(Some(Duration::from_millis(100)));
    match p.expect(Eof) {
        Err(expectrl::Error::ExpectTimeout) => {}
        r => panic!("should raise TimeOut {:?}", r),
    }
}

#[cfg(feature = "async")]
#[test]
fn expect_eof_timeout() {
    futures_lite::future::block_on(async {
        let mut p = spawn("sleep 3").expect("cannot run sleep 3");
        p.set_expect_timeout(Some(Duration::from_millis(100)));
        match p.expect(Eof).await {
            Err(expectrl::Error::ExpectTimeout) => {}
            r => panic!("should raise TimeOut {:?}", r),
        }
    })
}
