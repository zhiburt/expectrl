use expectrl::{spawn, Eof, NBytes, Regex};
use std::time::Duration;

#[cfg(not(feature = "async"))]
use std::io::Read;

#[cfg(feature = "async")]
use futures_lite::io::AsyncReadExt;

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn expect_str() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    session.expect("Hello World").unwrap();
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn expect_str() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        session.expect("Hello World").await.unwrap();
    })
}

#[cfg(windows)]
#[test]
fn expect_str() {
    let mut session = spawn("pwsh -C type").unwrap();

    // give shell some time
    std::thread::sleep(Duration::from_millis(300));

    #[cfg(not(feature = "async"))]
    {
        session.send_line("Hello World").unwrap();
        session.expect("Hello World").unwrap();
    }

    #[cfg(feature = "async")]
    {
        futures_lite::future::block_on(async {
            session.send_line("Hello World").await.unwrap();
            session.expect("Hello World").await.unwrap();
        })
    }
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn expect_regex() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    let m = session.expect(Regex("lo.*")).unwrap();
    assert_eq!(m.before(), b"Hel");
    assert_eq!(m.get(0).unwrap(), b"lo");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn expect_regex() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        let m = session.expect(Regex("lo.*")).await.unwrap();
        assert_eq!(m.before(), b"Hel");
        assert_eq!(m.get(0).unwrap(), b"lo");
    })
}

#[cfg(windows)]
#[test]
fn expect_regex() {
    let mut session = spawn("echo Hello World").unwrap();

    // give shell some time
    std::thread::sleep(Duration::from_millis(300));

    #[cfg(not(feature = "async"))]
    {
        let m = session.expect(Regex("lo.*")).unwrap();
        assert_eq!(
            m.before(),
            [27, 91, 50, 74, 27, 91, 109, 27, 91, 72, 72, 101, 108]
        );
        assert_eq!(m.get(0).unwrap(), b"lo");
    }

    #[cfg(feature = "async")]
    {
        futures_lite::future::block_on(async {
            let m = session.expect(Regex("lo.*")).await.unwrap();
            assert_eq!(
                m.before(),
                [27, 91, 50, 74, 27, 91, 109, 27, 91, 72, 72, 101, 108]
            );
            assert_eq!(m.get(0).unwrap(), b"lo");
        })
    }
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn expect_n_bytes() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    let m = session.expect(NBytes(3)).unwrap();
    assert_eq!(m.get(0).unwrap(), b"Hel");
    assert_eq!(m.before(), b"");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn expect_n_bytes() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        let m = session.expect(NBytes(3)).await.unwrap();
        assert_eq!(m.get(0).unwrap(), b"Hel");
        assert_eq!(m.before(), b"");
    })
}

#[cfg(windows)]
#[test]
fn expect_n_bytes() {
    use expectrl::{session::Session, ProcAttr};

    let mut session = Session::spawn(ProcAttr::cmd(r#"echo Hello World"#.to_string())).unwrap();

    // give shell some time
    std::thread::sleep(Duration::from_millis(300));

    #[cfg(not(feature = "async"))]
    {
        // ignore spawned command
        let m = session.expect(NBytes(14)).unwrap();
        assert_eq!(
            m.get(0).unwrap(),
            "\u{1b}[2J\u{1b}[m\u{1b}[HHell".as_bytes()
        );
        assert_eq!(m.before(), b"");
    }

    #[cfg(feature = "async")]
    {
        futures_lite::future::block_on(async {
            // ignore spawned command
            let m = session.expect(NBytes(14)).await.unwrap();
            assert_eq!(
                m.get(0).unwrap(),
                "\u{1b}[2J\u{1b}[m\u{1b}[HHell".as_bytes()
            );
            assert_eq!(m.before(), b"");
        })
    }
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn expect_eof() {
    let mut session = spawn("echo 'Hello World'").unwrap();
    session.set_expect_timeout(None);
    let m = session.expect(Eof).unwrap();
    assert_eq!(m.get(0).unwrap(), b"'Hello World'\r\n");
    assert_eq!(m.before(), b"");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn expect_eof() {
    futures_lite::future::block_on(async {
        let mut session = spawn("echo 'Hello World'").unwrap();
        session.set_expect_timeout(None);
        let m = session.expect(Eof).await.unwrap();
        assert_eq!(m.get(0).unwrap(), b"'Hello World'\r\n");
        assert_eq!(m.before(), b"");
    })
}

#[cfg(windows)]
#[test]
#[ignore = "https://stackoverflow.com/questions/68985384/does-a-conpty-reading-pipe-get-notified-on-process-termination"]
fn expect_eof() {
    let mut session = spawn("echo 'Hello World'").unwrap();

    // give shell some time
    std::thread::sleep(Duration::from_millis(300));

    #[cfg(not(feature = "async"))]
    {
        let m = session.expect(Eof).unwrap();
        assert_eq!(m.get(0).unwrap(), b"'Hello World'\r\n");
        assert_eq!(m.before(), b"");
    }

    #[cfg(feature = "async")]
    {
        futures_lite::future::block_on(async {
            let m = session.expect(Eof).await.unwrap();
            assert_eq!(m.get(0).unwrap(), b"'Hello World'\r\n");
            assert_eq!(m.before(), b"");
        })
    }
}

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(windows)]
#[cfg(not(feature = "async"))]
#[test]
fn read_after_expect_str() {
    let mut session = spawn("echo 'Hello World'").unwrap();

    // give shell some time
    std::thread::sleep(Duration::from_millis(300));

    session.expect("Hello").unwrap();

    let mut buf = [0; 6];
    session.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b" World");
}

#[cfg(windows)]
#[cfg(feature = "async")]
#[test]
fn read_after_expect_str() {
    let mut session = spawn("echo 'Hello World'").unwrap();

    // give shell some time
    std::thread::sleep(Duration::from_millis(300));

    futures_lite::future::block_on(async {
        session.expect("Hello").await.unwrap();

        let mut buf = [0; 6];
        session.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b" World");
    })
}

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(windows)]
#[test]
fn expect_eof_timeout() {
    let mut p = spawn("sleep 3").expect("cannot run sleep 3");
    p.set_expect_timeout(Some(Duration::from_millis(100)));

    #[cfg(not(feature = "async"))]
    {
        match p.expect(Eof) {
            Err(expectrl::Error::ExpectTimeout) => {}
            r => panic!("should raise TimeOut {:?}", r),
        }
    }

    #[cfg(feature = "async")]
    {
        futures_lite::future::block_on(async {
            match p.expect(Eof).await {
                Err(expectrl::Error::ExpectTimeout) => {}
                r => panic!("should raise TimeOut {:?}", r),
            }
        })
    }
}
