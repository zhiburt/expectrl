#![cfg(unix)]

use expectrl::{spawn, Any, Eof, NBytes, Regex, WaitStatus};
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
    assert_eq!(m.get(0).unwrap(), b"lo World\r");
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
        assert_eq!(m.get(0).unwrap(), b"lo World\r");
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
    assert_eq!(m.get(0).unwrap(), b"Hel");
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
        assert_eq!(m.get(0).unwrap(), b"Hel");
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
    assert_eq!(m.before(), b"");
    #[cfg(target_os = "linux")]
    assert_eq!(m.get(0).unwrap(), b"'Hello World'\r\n");

    #[cfg(not(target_os = "linux"))]
    {
        if m.matches().len() > 0 {
            assert_eq!(m.get(0).unwrap(), b"'Hello World'\r\n");
        }
    }
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn check_eof() {
    futures_lite::future::block_on(async {
        let mut session = spawn("echo 'Hello World'").unwrap();

        thread::sleep(Duration::from_millis(600));

        let m = session.check(Eof).await.unwrap();
        assert!(m.matches().count() == 0);

        let m = session.check(Eof).await.unwrap();
        assert_eq!(m.before(), b"");
        #[cfg(target_os = "linux")]
        assert_eq!(m.get(0).unwrap(), b"'Hello World'\r\n");
        #[cfg(not(target_os = "linux"))]
        assert!(m.matches().len() == 0);
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
        r => panic!("reached a timeout {r:?}"),
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
            r => panic!("reached a timeout {r:?}"),
        }
    })
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn check_macro() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();

    thread::sleep(Duration::from_millis(600));

    expectrl::check!(
        &mut session,
        world = "\r" => {
            assert_eq!(world.get(0).unwrap(), b"\r");
        },
        _ = "Hello World" => {
            panic!("Unexpected result");
        },
        default => {
            panic!("Unexpected result");
        },
    )
    .unwrap();
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn check_macro() {
    let mut session = spawn("cat").unwrap();
    futures_lite::future::block_on(session.send_line("Hello World")).unwrap();

    thread::sleep(Duration::from_millis(600));

    futures_lite::future::block_on(async {
        expectrl::check!(
            session,
            // world = "\r" => {
            //     assert_eq!(world.get(0).unwrap(), b"\r");
            // },
            // _ = "Hello World" => {
            //     panic!("Unexpected result");
            // },
            // default => {
            //     panic!("Unexpected result");
            // },
        )
        .await
        .unwrap();
    });
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn check_macro_eof() {
    let mut session = spawn("echo 'Hello World'").unwrap();

    assert_eq!(
        WaitStatus::Exited(session.pid(), 0),
        session.wait().unwrap()
    );

    expectrl::check!(
        &mut session,
        output = Eof => {
            #[cfg(target_os = "linux")]
            assert_eq!(output.get(0).unwrap(), b"'Hello World'\r\n");
            #[cfg(not(target_os = "linux"))]
            assert_eq!(output.get(0).unwrap(), b"");
            assert_eq!(output.before(), b"");
        },
        default => {
            panic!("Unexpected result");
        },
    )
    .unwrap();
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn check_macro_eof() {
    let mut session = spawn("echo 'Hello World'").unwrap();

    assert_eq!(
        WaitStatus::Exited(session.pid(), 0),
        session.wait().unwrap()
    );

    futures_lite::future::block_on(async {
        let m = session.check(Eof).await.unwrap();
        assert!(m.matches().next().is_none());

        expectrl::check!(
            session,
            output = Eof => {
                #[cfg(target_os = "linux")]
                assert_eq!(output.get(0).unwrap(), b"'Hello World'\r\n");
                #[cfg(not(target_os = "linux"))]
                assert_eq!(output.get(0).unwrap(), b"");
                assert_eq!(output.before(), b"");
            },
            default => {
                panic!("Unexpected result");
            },
        )
        .await
        .unwrap();
    });
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn check_macro_doest_consume_missmatch() {
    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();
    thread::sleep(Duration::from_millis(600));

    expectrl::check!(
        &mut session,
        _ = "Something which is not inside" => {
            panic!("Unexpected result");
        },
    )
    .unwrap();

    session.send_line("345").unwrap();
    thread::sleep(Duration::from_millis(600));

    expectrl::check!(
        &mut session,
        buffer = Eof => {
            assert_eq!(buffer.get(0).unwrap(), b"Hello World\r\n")
        },
    )
    .unwrap();
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn check_macro_doest_consume_missmatch() {
    let mut session = spawn("cat").unwrap();

    futures_lite::future::block_on(async {
        session.send_line("Hello World").await.unwrap();
        thread::sleep(Duration::from_millis(600));

        expectrl::check!(
            session,
            _ = "Something which is not inside" => {
                panic!("Unexpected result");
            },
        )
        .await
        .unwrap();

        session.send_line("345").await.unwrap();
        thread::sleep(Duration::from_millis(600));

        expectrl::check!(
            session,
            buffer = Eof => {
                assert_eq!(buffer.get(0).unwrap(), b"Hello World\r\n")
            },
        )
        .await
        .unwrap();
    });
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
#[test]
fn check_macro_with_different_needles() {
    let check_input = |session: &mut _| {
        expectrl::check!(
            session,
            number = Any(["123", "345"]) => {
                assert_eq!(number.get(0).unwrap(), b"345")
            },
            line = "\n" => {
                assert_eq!(line.before(), b"Hello World\r")
            },
            default => {
                panic!("Unexpected result");
            },
        )
        .unwrap();
    };

    let mut session = spawn("cat").unwrap();
    session.send_line("Hello World").unwrap();

    thread::sleep(Duration::from_millis(600));
    check_input(&mut session);

    session.send_line("345").unwrap();

    thread::sleep(Duration::from_millis(600));
    check_input(&mut session);
}

#[cfg(unix)]
#[cfg(feature = "async")]
#[test]
fn check_macro_with_different_needles() {
    futures_lite::future::block_on(async {
        let mut session = spawn("cat").unwrap();
        session.send_line("Hello World").await.unwrap();

        thread::sleep(Duration::from_millis(600));
        expectrl::check!(
            session,
            number = Any(["123", "345"]) => {
                assert_eq!(number.get(0).unwrap(), b"345")
            },
            line = "\n" => {
                assert_eq!(line.before(), b"Hello World\r")
            },
            default => {
                panic!("Unexpected result");
            },
        )
        .await
        .unwrap();

        session.send_line("345").await.unwrap();

        thread::sleep(Duration::from_millis(600));
        expectrl::check!(
            session,
            number = Any(["123", "345"]) => {
                assert_eq!(number.get(0).unwrap(), b"345")
            },
            line = "\n" => {
                assert_eq!(line.before(), b"Hello World\r")
            },
            default => {
                panic!("Unexpected result");
            },
        )
        .await
        .unwrap();
    });
}
