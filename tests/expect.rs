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
    println!("{:?}", std::fs::metadata("./tests/actions/cat/main.py"));
    println!(
        "{:?}",
        std::process::Command::new("python")
            .args(["./tests/actions/echo/main.py", "1231", "1231xx"])
            .output()
    );

    use std::io::Read;
    let mut session = spawn("python ./tests/actions/cat/main.py").unwrap();
    eprintln!("{:?}", session.get_process().pid());
    eprintln!("{:?}", session.is_alive());
    eprintln!("{:?}", session.is_empty());
    session.send_line("Hello World\n\r").unwrap();
    eprintln!("{:?}", session.is_alive());
    eprintln!("{:?}", session.is_empty());

    let mut buf = vec![0; 200];
    println!("xx {:?}", session.read(&mut buf));
    eprintln!("xx {:?}", String::from_utf8_lossy(&buf));

    let mut session = spawn("python ./tests/actions/cat/main.py").unwrap();

    #[cfg(not(feature = "async"))]
    {
        session.send_line("Hello World\n\r").unwrap();
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
    assert_eq!(m.get(0).unwrap(), b"lo World\r");
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn expect_regex_lazy() {
    let mut session = spawn("cat").unwrap();
    session.set_expect_lazy(true);
    session.send_line("Hello World").unwrap();
    let m = session.expect(Regex("lo.*")).unwrap();
    assert_eq!(m.before(), b"Hel");
    assert_eq!(m.get(0).unwrap(), b"lo");
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn expect_gready_regex() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();
        let m = session.expect(Regex("lo.*")).await.unwrap();
        assert_eq!(m.before(), b"Hel");
        assert_eq!(m.get(0).unwrap(), b"lo World\r");
    })
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn expect_lazy_regex() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.set_expect_lazy(true);
        session.send_line("Hello World").await.unwrap();
        let m = session.expect(Regex("lo.*")).await.unwrap();
        assert_eq!(m.before(), b"Hel");
        assert_eq!(m.get(0).unwrap(), b"lo");
    })
}

#[cfg(windows)]
#[test]
fn expect_regex() {
    let mut session = spawn("python ./tests/actions/echo/main.py Hello World").unwrap();
    #[cfg(not(feature = "async"))]
    {
        let m = session.expect(Regex("lo.*")).unwrap();
        assert_eq!(m.matches().count(), 1);
        assert_eq!(m.get(0).unwrap(), b"lo World\r");
    }

    #[cfg(feature = "async")]
    {
        futures_lite::future::block_on(async {
            let m = session.expect(Regex("lo.*")).await.unwrap();
            assert_eq!(m.matches().count(), 1);
            assert_eq!(m.get(0).unwrap(), b"lo World\r");
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
    use expectrl::Session;
    use std::process::Command;

    let mut session = Session::spawn(Command::new(
        "python ./tests/actions/echo/main.py Hello World",
    ))
    .unwrap();
    #[cfg(not(feature = "async"))]
    {
        let m = session.expect(NBytes(14)).unwrap();
        assert_eq!(m.matches().count(), 1);
        assert_eq!(m.get(0).unwrap().len(), 14);
        assert_eq!(m.before(), b"");
    }

    #[cfg(feature = "async")]
    {
        futures_lite::future::block_on(async {
            let m = session.expect(NBytes(14)).await.unwrap();
            assert_eq!(m.matches().count(), 1);
            assert_eq!(m.get(0).unwrap().len(), 14);
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
        r => panic!("reached a timeout {r:?}"),
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
            r => panic!("reached a timeout {r:?}"),
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
