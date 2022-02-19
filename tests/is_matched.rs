#![cfg(unix)]

use expectrl::{spawn, Eof, NBytes, Regex, WaitStatus};
use std::thread;
use std::time::Duration;

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn is_matched_str() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    thread::sleep(Duration::from_millis(600));
    assert!(session.is_matched("Hello World").unwrap());
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn is_matched_str() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        thread::sleep(Duration::from_millis(600));
        assert!(session.is_matched("Hello World").await.unwrap());
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn is_matched_regex() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();

    thread::sleep(Duration::from_millis(600));

    assert!(session.is_matched(Regex("lo.*")).unwrap());
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn is_matched_regex() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();

        thread::sleep(Duration::from_millis(600));

        assert!(session.is_matched(Regex("lo.*")).await.unwrap());
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn is_matched_bytes() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();

    thread::sleep(Duration::from_millis(600));

    assert!(session.is_matched(NBytes(3)).unwrap());
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn is_matched_n_bytes() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();

        thread::sleep(Duration::from_millis(600));

        assert!(session.is_matched(NBytes(3)).await.unwrap());
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn is_matched_eof() {
    let mut session = spawn("echo 'Hello World'").unwrap();

    assert_eq!(
        WaitStatus::Exited(session.pid(), 0),
        session.wait().unwrap()
    );

    assert!(session.is_matched(Eof).unwrap());
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn is_matched_eof() {
    futures_lite::future::block_on(async {
        let mut session = spawn("echo 'Hello World'").unwrap();

        assert_eq!(
            WaitStatus::Exited(session.pid(), 0),
            session.wait().unwrap()
        );

        assert!(!session.is_matched(Eof).await.unwrap());
        assert!(session.is_matched(Eof).await.unwrap());
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn read_after_is_matched() {
    use std::io::Read;

    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();

    thread::sleep(Duration::from_millis(600));

    assert!(session.is_matched("Hello").unwrap());

    // we stop process so read operation will end up with EOF.
    // other wise read call would block.
    session.exit(false).unwrap();

    let mut buf = [0; 128];
    let n = session.read(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"Hello World\r\n");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn read_after_is_matched() {
    use futures_lite::io::AsyncReadExt;

    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();

        thread::sleep(Duration::from_millis(600));

        assert!(session.is_matched("Hello").await.unwrap());

        // we stop process so read operation will end up with EOF.
        // other wise read call would block.
        session.exit(false).unwrap();

        let mut buf = [0; 128];
        let n = session.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"Hello World\r\n");
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn check_after_is_matched_eof() {
    let mut p = spawn("echo AfterSleep").expect("cannot run echo");
    assert_eq!(WaitStatus::Exited(p.pid(), 0), p.wait().unwrap());
    assert!(p.is_matched(Eof).unwrap());

    let m = p.check(Eof).unwrap();

    #[cfg(target_os = "linux")]
    assert_eq!(m.matches()[0], b"AfterSleep\r\n");

    #[cfg(not(target_os = "linux"))]
    assert_eq!(m.matches()[0], b"");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn check_after_is_matched_eof() {
    futures_lite::future::block_on(async {
        let mut p = spawn("echo AfterSleep").expect("cannot run echo");
        assert_eq!(WaitStatus::Exited(p.pid(), 0), p.wait().unwrap());

        assert!(!p.is_matched(Eof).await.unwrap());
        assert!(p.is_matched(Eof).await.unwrap());

        let m = p.check(Eof).await.unwrap();

        #[cfg(target_os = "linux")]
        assert_eq!(m.matches()[0], b"AfterSleep\r\n");

        #[cfg(not(target_os = "linux"))]
        assert!(m.matches().is_empty());
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn expect_after_is_matched_eof() {
    let mut p = spawn("echo AfterSleep").expect("cannot run echo");
    assert_eq!(WaitStatus::Exited(p.pid(), 0), p.wait().unwrap());
    assert!(p.is_matched(Eof).unwrap());

    let m = p.expect(Eof).unwrap();

    #[cfg(target_os = "linux")]
    assert_eq!(m.matches()[0], b"AfterSleep\r\n");

    #[cfg(not(target_os = "linux"))]
    assert_eq!(m.matches()[0], b"");

    assert!(matches!(p.expect("").unwrap_err(), expectrl::Error::Eof));
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn expect_after_is_matched_eof() {
    futures_lite::future::block_on(async {
        let mut p = spawn("echo AfterSleep").expect("cannot run echo");
        assert_eq!(WaitStatus::Exited(p.pid(), 0), p.wait().unwrap());

        assert!(!p.is_matched(Eof).await.unwrap());
        assert!(p.is_matched(Eof).await.unwrap());

        let m = p.expect(Eof).await.unwrap();

        #[cfg(target_os = "linux")]
        assert_eq!(m.matches()[0], b"AfterSleep\r\n");

        #[cfg(not(target_os = "linux"))]
        assert!(m.matches().is_empty());

        assert!(matches!(
            p.expect("").await.unwrap_err(),
            expectrl::Error::Eof
        ));
    })
}
