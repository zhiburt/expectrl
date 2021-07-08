#![cfg(feature = "sync")]

use expectrl::{Eof, NBytes, Regex, Session};
use std::{
    io::{Read, Write},
    thread,
    time::Duration,
};

#[test]
fn send() {
    let mut session = Session::spawn("cat").unwrap();
    session.send("Hello World").unwrap();

    thread::sleep(Duration::from_millis(300));
    session.write_all(&[3]).unwrap(); // Ctrl+C
    session.flush().unwrap();

    let mut buf = String::new();
    session.read_to_string(&mut buf).unwrap();

    // cat doesn't printed anything
    assert_eq!(buf, "");
}

#[test]
fn send_multiline() {
    let mut session = Session::spawn("cat").unwrap();
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

#[test]
fn send_line() {
    let mut session = Session::spawn("cat").unwrap();
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

#[test]
fn expect_str() {
    let mut session = Session::spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    session.expect("Hello World").unwrap();
}

#[test]
fn expect_regex() {
    let mut session = Session::spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    let m = session.expect(Regex("lo.*")).unwrap();
    assert_eq!(m.before_match(), b"Hel");
    assert_eq!(m.found_match(), b"lo");
}

#[test]
fn expect_n_bytes() {
    let mut session = Session::spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    let m = session.expect(NBytes(3)).unwrap();
    assert_eq!(m.found_match(), b"Hel");
    assert_eq!(m.before_match(), b"");
}

#[test]
fn expect_eof() {
    let mut session = Session::spawn("echo 'Hello World'").unwrap();
    session.send_line("Hello World").unwrap();
    session.set_expect_timeout(None);
    let m = session.expect(Eof).unwrap();
    assert_eq!(m.found_match(), b"'Hello World'\r\n");
    assert_eq!(m.before_match(), b"");
}

#[test]
fn read_after_expect_str() {
    let mut session = Session::spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    session.expect("Hello").unwrap();

    let mut buf = [0; 6];
    session.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b" World");
}

#[test]
fn expect_eof_timeout() {
    let mut p = Session::spawn("sleep 3").expect("cannot run sleep 3");
    p.set_expect_timeout(Some(Duration::from_millis(100)));
    match p.expect(Eof) {
        Err(expectrl::Error::ExpectTimeout) => {}
        r => panic!("should raise TimeOut {:?}", r),
    }
}
