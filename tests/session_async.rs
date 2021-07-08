#![cfg(feature = "async")]

use expectrl::{Eof, NBytes, Regex, Session};
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
use std::{thread, time::Duration};

#[test]
fn send() {
    futures_lite::future::block_on(async {
        let mut session = Session::spawn("cat").unwrap();
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

#[test]
fn send_multiline() {
    futures_lite::future::block_on(async {
        let mut session = Session::spawn("cat").unwrap();
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

#[test]
fn send_line() {
    futures_lite::future::block_on(async {
        let mut session = Session::spawn("cat").unwrap();
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

#[test]
fn expect_str() {
    futures_lite::future::block_on(async {
        let mut session = Session::spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        session.expect("Hello World").await.unwrap();
    })
}

#[test]
fn expect_regex() {
    futures_lite::future::block_on(async {
        let mut session = Session::spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        let m = session.expect(Regex("lo.*")).await.unwrap();
        assert_eq!(m.before_match(), b"Hel");
        assert_eq!(m.found_match(), b"lo");
    })
}

#[test]
fn expect_n_bytes() {
    futures_lite::future::block_on(async {
        let mut session = Session::spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        let m = session.expect(NBytes(3)).await.unwrap();
        assert_eq!(m.found_match(), b"Hel");
        assert_eq!(m.before_match(), b"");
    })
}

#[test]
fn expect_eof() {
    futures_lite::future::block_on(async {
        let mut session = Session::spawn("echo 'Hello World'").unwrap();
        session.send_line("Hello World").await.unwrap();
        session.set_expect_timeout(None);
        let m = session.expect(Eof).await.unwrap();
        assert_eq!(m.found_match(), b"'Hello World'\r\n");
        assert_eq!(m.before_match(), b"");
    })
}

#[test]
fn read_after_expect_str() {
    futures_lite::future::block_on(async {
        let mut session = Session::spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        session.expect("Hello").await.unwrap();

        let mut buf = [0; 6];
        session.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b" World");
    })
}

#[test]
fn expect_eof_timeout() {
    futures_lite::future::block_on(async {
        let mut p = Session::spawn("sleep 3").expect("cannot run sleep 3");
        p.set_expect_timeout(Some(Duration::from_millis(100)));
        match p.expect(Eof).await {
            Err(expectrl::Error::ExpectTimeout) => {}
            r => panic!("should raise TimeOut {:?}", r),
        }
    })
}
