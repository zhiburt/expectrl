use expectrl::spawn;

#[cfg(unix)]
use std::{thread, time::Duration};

#[cfg(feature = "async")]
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(not(feature = "async"))]
use std::io::{Read, Write};

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
    session.write(b"Hello World").unwrap();

    let mut buf = vec![0; 1028];
    let _ = session.read(&mut buf).unwrap();
    let n = session.read(&mut buf).unwrap();

    assert!(String::from_utf8_lossy(&buf[..n]).contains("Hello World"));
}

#[cfg(unix)]
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
    session.send("Hello World\r\n").unwrap();

    let buf = session.lines().nth(2).unwrap().unwrap();

    println!("{}", buf);
    assert!(buf.contains("Hello World"));
}

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(windows)]
#[test]
fn send_line() {
    let mut session = spawn("powershell -C type").unwrap();
    session.send_line("Hello World").unwrap();

    let buf = session.lines().nth(2).unwrap().unwrap();

    println!("{}", buf);
    assert!(buf.contains("Hello World"));
}
