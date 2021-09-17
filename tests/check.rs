use expectrl::{spawn, Eof, NBytes, Regex};
use std::thread;
use std::time::Duration;

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn check_str() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    session.check("Hello World").unwrap();
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn check_str() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        session.check("Hello World").await.unwrap();
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn check_regex() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();

    thread::sleep(Duration::from_millis(600));

    let m = session.check(Regex("lo.*")).unwrap();
    assert_eq!(m.before(), b"Hel");
    assert_eq!(m.first(), b"lo World\r");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn check_regex() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();

        thread::sleep(Duration::from_millis(600));

        let m = session.check(Regex("lo.*")).await.unwrap();
        assert_eq!(m.before(), b"Hel");
        assert_eq!(m.first(), b"lo World\r");
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn check_n_bytes() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();

    thread::sleep(Duration::from_millis(600));

    let m = session.check(NBytes(3)).unwrap();
    assert_eq!(m.first(), b"Hel");
    assert_eq!(m.before(), b"");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn check_n_bytes() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();

        thread::sleep(Duration::from_millis(600));

        let m = session.check(NBytes(3)).await.unwrap();
        assert_eq!(m.first(), b"Hel");
        assert_eq!(m.before(), b"");
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn check_eof() {
    let mut session = spawn("echo 'Hello World'").unwrap();

    thread::sleep(Duration::from_millis(600));

    let m = session.check(Eof).unwrap();
    assert_eq!(m.first(), b"'Hello World'\r\n");
    assert_eq!(m.before(), b"");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn check_eof() {
    futures_lite::future::block_on(async {
        let mut session = spawn("echo 'Hello World'").unwrap();

        thread::sleep(Duration::from_millis(600));

        let m = session.check(Eof).await.unwrap();
        assert_eq!(m.first(), b"'Hello World'\r\n");
        assert_eq!(m.before(), b"");
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn read_after_check_str() {
    use std::io::Read;

    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();

    thread::sleep(Duration::from_millis(600));

    let f = session.check("Hello").unwrap();
    println!("{:?}", f);
    assert!(!f.is_empty());

    // we stop process so read operation will fail.
    // other wise read call would block.
    session.exit(false).unwrap();

    let mut buf = [0; 6];
    session.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b" World");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn read_after_check_str() {
    use futures_lite::io::AsyncReadExt;

    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();

        thread::sleep(Duration::from_millis(600));

        let f = session.check("Hello").await.unwrap();
        println!("{:?}", f);
        assert!(!f.is_empty());

        // we stop process so read operation will fail.
        // other wise read call would block.
        session.exit(false).unwrap();

        let mut buf = [0; 6];
        session.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b" World");
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn check_eof_timeout() {
    let mut p = spawn("sleep 3").expect("cannot run sleep 3");
    match p.check(Eof) {
        Ok(found) if found.is_empty() => {}
        r => panic!("should raise TimeOut {:?}", r),
    }
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn check_eof_timeout() {
    futures_lite::future::block_on(async {
        let mut p = spawn("sleep 3").expect("cannot run sleep 3");
        match p.check(Eof).await {
            Ok(found) if found.is_empty() => {}
            r => panic!("should raise TimeOut {:?}", r),
        }
    })
}
