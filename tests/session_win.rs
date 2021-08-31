#![cfg(windows)]

use expectrl::{spawn, Eof, NBytes, Regex};
use std::{thread, time::Duration};
use std::io::{BufRead, Read, Write};

#[test]
fn send() {
    let mut session = spawn("powershell -C type").unwrap();
    session.send("Hello World").unwrap();

    let mut buf = vec![0; 1028];
    let _ = session.read(&mut buf).unwrap();
    let n = session.read(&mut buf).unwrap();

    assert!(String::from_utf8_lossy(&buf[..n]).contains("Hello World"));
}

#[test]
fn send_multiline() {
    let mut session = spawn("powershell -C type").unwrap();
    session.send("Hello World\r\n").unwrap();

    let buf = session.lines().nth(2).unwrap().unwrap();

    println!("{}", buf);
    assert!(buf.contains("Hello World"));
}

#[test]
#[ignore = "write_vectored not properly implemented for conpty::Proc yet"]
fn send_line() {
    let mut session = spawn("powershell -C type").unwrap();
    session.send_line("Hello World").unwrap();

    let buf = session.lines().nth(2).unwrap().unwrap();

    println!("{}", buf);
    assert!(buf.contains("Hello World"));
}

#[test]
fn expect_str() {
    let mut session = spawn("powershell -C type").unwrap();
    session.send_line("Hello World").unwrap();
    session.expect("Hello World").unwrap();
}

#[test]
fn expect_regex() {
    let mut session = spawn("echo Hello World").unwrap();
    let m = session.expect(Regex("lo.*")).unwrap();
    assert_eq!(m.before_match(), [27, 91, 50, 74, 27, 91, 109, 27, 91, 72, 72, 101, 108]);
    assert_eq!(m.found_match(), b"lo");
}

#[test]
fn expect_n_bytes() {
    let mut session = spawn("echo 'Hello World'").unwrap();
    // ignore spawned command
    let m = session.expect(NBytes(14)).unwrap();
    println!("{:?}", String::from_utf8_lossy(m.found_match()));
    assert_eq!(m.found_match(), "\u{1b}[2J\u{1b}[m\u{1b}[H'Hel".as_bytes());
    assert_eq!(m.before_match(), b"");
}

#[test]
#[ignore = "https://stackoverflow.com/questions/68985384/does-a-conpty-reading-pipe-get-notified-on-process-termination"]
fn expect_eof() {
    let mut session = spawn("echo 'Hello World'").unwrap();
    let m = session.expect(Eof).unwrap();
    assert_eq!(m.found_match(), b"'Hello World'\r\n");
    assert_eq!(m.before_match(), b"");
}

#[test]
fn read_after_expect_str() {
    let mut session = spawn("echo 'Hello World'").unwrap();
    session.expect("Hello").unwrap();

    let mut buf = [0; 6];
    session.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b" World");
}

#[test]
fn expect_eof_timeout() {
    let mut p = spawn("sleep 3").expect("cannot run sleep 3");
    p.set_expect_timeout(Some(Duration::from_millis(100)));
    match p.expect(Eof) {
        Err(expectrl::Error::ExpectTimeout) => {}
        r => panic!("should raise TimeOut {:?}", r),
    }
}
