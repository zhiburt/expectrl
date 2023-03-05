use expectrl::{Captures, ControlCode, Needle, Session};
use std::{process::Command, thread, time::Duration};

#[cfg(unix)]
use expectrl::WaitStatus;

#[cfg(feature = "async")]
use futures_lite::{
    future::block_on,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt},
};

#[cfg(not(feature = "async"))]
use std::io::{BufRead, Read, Write};

#[test]
#[cfg(unix)]
fn send_controll() {
    let mut proc = Session::spawn(Command::new("cat")).unwrap();
    _p_send_control(&mut proc, ControlCode::EOT).unwrap();
    assert_eq!(proc.wait().unwrap(), WaitStatus::Exited(proc.pid(), 0),);
}

#[test]
#[cfg(windows)]
fn send_controll() {
    let mut proc = Session::spawn(Command::new("powershell -C ping localhost")).unwrap();

    // give powershell a bit time
    thread::sleep(Duration::from_millis(100));

    _p_send_control(&mut proc, ControlCode::ETX).unwrap();
    assert!({
        let code = proc.wait(None).unwrap();
        code == 0 || code == 3221225786
    });
}

#[test]
#[cfg(unix)]
fn send() {
    let mut proc = Session::spawn(Command::new("cat")).unwrap();
    _p_send(&mut proc, "hello cat\n").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    let mut buf = vec![0; 128];
    let n = _p_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"hello cat\r\n");

    assert!(proc.exit(true).unwrap());
}

#[test]
#[cfg(windows)]
fn send() {
    let mut proc = Session::spawn(Command::new("powershell -C type")).unwrap();
    thread::sleep(Duration::from_millis(1000));

    _p_send(&mut proc, "hello cat\r\n").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(600));

    _p_expect(&mut proc, "hello cat").unwrap();
    proc.exit(0).unwrap();
}

#[test]
#[cfg(unix)]
fn send_line() {
    let mut proc = Session::spawn(Command::new("cat")).unwrap();

    _p_send_line(&mut proc, "hello cat").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    let mut buf = vec![0; 128];
    let n = _p_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"hello cat\r\n");

    assert!(proc.exit(true).unwrap());
}

#[test]
#[cfg(windows)]
fn send_line() {
    let mut proc = Session::spawn(Command::new("powershell -C type")).unwrap();

    thread::sleep(Duration::from_millis(1000));
    _p_send_line(&mut proc, "hello cat").unwrap();
    thread::sleep(Duration::from_millis(1000));

    _p_expect(&mut proc, "hello cat").unwrap();
    proc.exit(0).unwrap();
}

#[test]
#[cfg(unix)]
fn try_read_by_byte() {
    let mut proc = Session::spawn(Command::new("cat")).unwrap();

    assert_eq!(
        _p_try_read(&mut proc, &mut [0; 1]).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );

    _p_send_line(&mut proc, "123").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    let mut buf = [0; 1];
    _p_try_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf, &[b'1']);
    _p_try_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf, &[b'2']);
    _p_try_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf, &[b'3']);
    _p_try_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf, &[b'\r']);
    _p_try_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf, &[b'\n']);
    assert_eq!(
        _p_try_read(&mut proc, &mut buf).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
#[cfg(windows)]
#[cfg(not(feature = "async"))]
fn try_read_by_byte() {
    // it shows that on windows ECHO is turned on.
    // Mustn't it be turned down?

    let mut proc = Session::spawn(Command::new("powershell")).unwrap();
    _p_send_line(
        &mut proc,
        "while (1) { read-host | set r; if (!$r) { break }}",
    )
    .unwrap();
    _p_read_until(&mut proc, b'}').unwrap();
    _p_read_line(&mut proc).unwrap();

    _p_send_line(&mut proc, "123").unwrap();

    thread::sleep(Duration::from_millis(500));

    _p_read_until(&mut proc, b'1').unwrap();

    let mut buf = [0; 1];
    _p_try_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf, &[b'2']);
    _p_try_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf, &[b'3']);
    _p_try_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf, &[b'\r']);
    _p_try_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf, &[b'\n']);
}

#[test]
#[cfg(unix)]
fn blocking_read_after_non_blocking() {
    let mut proc = Session::spawn(Command::new("cat")).unwrap();

    assert!(_p_is_empty(&mut proc).unwrap());

    _p_send_line(&mut proc, "123").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    let mut buf = [0; 1];
    _p_try_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf, &[b'1']);

    let mut buf = [0; 64];
    let n = _p_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"23\r\n");

    thread::spawn(move || {
        let _ = _p_read(&mut proc, &mut buf).unwrap();
        // the error will be propagated in case of panic
        panic!("it's unnexpected that read operation will be ended")
    });

    // give some time to read
    thread::sleep(Duration::from_millis(100));
}

#[test]
#[cfg(windows)]
fn blocking_read_after_non_blocking() {
    let mut proc = Session::spawn(Command::new("powershell")).unwrap();
    _p_send_line(
        &mut proc,
        "while (1) { read-host | set r; if (!$r) { break }}",
    )
    .unwrap();

    thread::sleep(Duration::from_millis(300));

    _p_send_line(&mut proc, "123").unwrap();

    thread::sleep(Duration::from_millis(1000));

    assert!(do_until(
        || {
            thread::sleep(Duration::from_millis(50));
            _p_try_read(&mut proc, &mut [0; 1]).is_ok()
        },
        Duration::from_secs(3)
    ));

    let mut buf = [0; 64];
    let n = _p_read(&mut proc, &mut buf).unwrap();
    assert!(n > 0);
}

#[test]
#[cfg(unix)]
fn try_read() {
    let mut proc = Session::spawn(Command::new("cat")).unwrap();

    let mut buf = vec![0; 128];
    assert_eq!(
        _p_try_read(&mut proc, &mut buf).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );

    _p_send_line(&mut proc, "123").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    assert_eq!(_p_try_read(&mut proc, &mut buf).unwrap(), 5);
    assert_eq!(&buf[..5], b"123\r\n");
    assert_eq!(
        _p_try_read(&mut proc, &mut buf).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
#[cfg(windows)]
fn try_read() {
    let mut proc = Session::spawn(Command::new("powershell")).unwrap();
    thread::sleep(Duration::from_millis(300));
    _p_send_line(
        &mut proc,
        "while (1) { read-host | set r; if (!$r) { break }}",
    )
    .unwrap();

    thread::sleep(Duration::from_millis(500));

    _p_send_line(&mut proc, "123").unwrap();
    _p_send_line(&mut proc, "123").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(1500));

    assert!(do_until(
        || {
            thread::sleep(Duration::from_millis(50));

            let mut buf = vec![0; 128];
            let _ = _p_try_read(&mut proc, &mut buf);

            if String::from_utf8_lossy(&buf).contains("123") {
                true
            } else {
                false
            }
        },
        Duration::from_secs(5)
    ));
}

#[test]
#[cfg(unix)]
fn blocking_read_after_non_blocking_try_read() {
    let mut proc = Session::spawn(Command::new("cat")).unwrap();

    let mut buf = vec![0; 1];
    assert_eq!(
        _p_try_read(&mut proc, &mut buf).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );

    _p_send_line(&mut proc, "123").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    assert_eq!(_p_try_read(&mut proc, &mut buf).unwrap(), 1);
    assert_eq!(&buf[..1], b"1");

    let mut buf = [0; 64];
    let n = _p_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"23\r\n");

    thread::spawn(move || {
        let _ = _p_read(&mut proc, &mut buf).unwrap();
        // the error will be propagated in case of panic
        panic!("it's unnexpected that read operation will be ended")
    });

    // give some time to read
    thread::sleep(Duration::from_millis(100));
}

#[cfg(unix)]
#[test]
fn try_read_after_eof() {
    let mut proc = Session::spawn(Command::new("cat")).unwrap();

    _p_send_line(&mut proc, "hello").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    let mut buf = vec![0; 128];
    assert_eq!(_p_try_read(&mut proc, &mut buf).unwrap(), 7);
    assert_eq!(
        _p_try_read(&mut proc, &mut buf).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(_p_is_empty(&mut proc).unwrap());
}

#[test]
#[cfg(unix)]
fn try_read_after_process_exit() {
    let mut command = Command::new("echo");
    command.arg("hello cat");
    let mut proc = Session::spawn(command).unwrap();

    assert_eq!(proc.wait().unwrap(), WaitStatus::Exited(proc.pid(), 0));

    #[cfg(target_os = "linux")]
    assert_eq!(_p_try_read(&mut proc, &mut [0; 128]).unwrap(), 11);

    #[cfg(not(target_os = "linux"))]
    assert_eq!(_p_try_read(&mut proc, &mut [0; 128]).unwrap(), 0);

    assert_eq!(_p_try_read(&mut proc, &mut [0; 128]).unwrap(), 0);
    assert!(_p_is_empty(&mut proc).unwrap());

    // // on macos we may not able to read after process is dead.
    // // I assume that kernel consumes proceses resorces without any code check of parent,
    // // which what is happening on linux.
    // //
    // // So we check that there may be None or Some(0)

    // // on macos we can't put it before read's for some reason something get blocked
    // // assert_eq!(proc.wait().unwrap(), WaitStatus::Exited(proc.pid(), 0));
}

#[cfg(windows)]
#[test]
fn try_read_after_process_exit() {
    use std::io::ErrorKind;

    let mut proc = Session::spawn(Command::new("cmd /C echo hello cat")).unwrap();

    assert_eq!(proc.wait(None).unwrap(), 0);

    let now = std::time::Instant::now();

    loop {
        if now.elapsed() > Duration::from_secs(2) {
            panic!("didn't read what expected")
        }

        match _p_try_read(&mut proc, &mut [0; 128]) {
            Ok(n) => {
                assert_eq!(n, 59);
                assert!(_p_try_read(&mut proc, &mut [0; 128]).is_err());
                assert!(_p_try_read(&mut proc, &mut [0; 128]).is_err());
                assert!(_p_is_empty(&mut proc).unwrap());
                assert_eq!(proc.wait(None).unwrap(), 0);
                return;
            }
            Err(err) => {
                if err.kind() == ErrorKind::WouldBlock {
                    continue;
                }

                panic!("unexpected error {:?}", err);
            }
        }
    }
}

#[test]
#[cfg(unix)]
fn try_read_to_end() {
    let mut cmd = Command::new("echo");
    cmd.arg("Hello World");
    let mut proc = Session::spawn(cmd).unwrap();

    let mut buf: Vec<u8> = Vec::new();
    loop {
        let mut b = [0; 128];
        match _p_try_read(&mut proc, &mut b) {
            Ok(0) => break,
            Ok(n) => buf.extend(&b[..n]),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(err) => Err(err).unwrap(),
        }
    }

    assert_eq!(&buf[..13], b"Hello World\r\n");
}

#[test]
#[cfg(windows)]
fn try_read_to_end() {
    let mut proc = Session::spawn(Command::new("cmd /C echo Hello World")).unwrap();

    let mut buf: Vec<u8> = Vec::new();

    let now = std::time::Instant::now();

    while now.elapsed() < Duration::from_secs(1) {
        let mut b = [0; 1];
        match _p_try_read(&mut proc, &mut b) {
            Ok(n) => buf.extend(&b[..n]),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => (),
            Err(err) => Err(err).unwrap(),
        }
    }

    assert!(String::from_utf8_lossy(&buf).contains("Hello World"));
}

#[test]
#[cfg(windows)]
fn continues_try_reads() {
    let cmd = Command::new("python3 -c \"import time; print('Start Sleep'); time.sleep(0.1); print('End of Sleep'); yn=input('input');\"");
    let mut proc = Session::spawn(cmd).unwrap();

    let mut buf = [0; 128];
    loop {
        if !proc.is_alive() {
            panic!("Most likely python is not installed");
        }

        match _p_try_read(&mut proc, &mut buf) {
            Ok(n) => {
                if String::from_utf8_lossy(&buf[..n]).contains("input") {
                    break;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(err) => Err(err).unwrap(),
        }
    }
}

#[test]
#[cfg(not(target_os = "macos"))]
#[cfg(not(windows))]
fn automatic_stop_of_interact_on_eof() {
    let mut p = Session::spawn(Command::new("ls")).unwrap();
    _p_interact(&mut p).unwrap();

    // check that second spawn works
    let mut p = Session::spawn(Command::new("ls")).unwrap();
    _p_interact(&mut p).unwrap();
}

#[test]
#[cfg(not(target_os = "macos"))]
#[cfg(not(windows))]
fn spawn_after_interact() {
    let mut p = Session::spawn(Command::new("ls")).unwrap();
    _p_interact(&mut p).unwrap();

    let p = Session::spawn(Command::new("ls")).unwrap();
    assert!(matches!(p.wait().unwrap(), WaitStatus::Exited(_, 0)));
}

#[test]
#[cfg(unix)]
fn read_line_test() {
    let mut proc = Session::spawn(Command::new("cat")).unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    _p_send_line(&mut proc, "123").unwrap();

    thread::sleep(Duration::from_millis(100));

    let line = _p_read_line(&mut proc).unwrap();
    assert_eq!(&line, "123\r\n");

    proc.exit(true).unwrap();
}

fn _p_read(proc: &mut Session, buf: &mut [u8]) -> std::io::Result<usize> {
    #[cfg(not(feature = "async"))]
    {
        proc.read(buf)
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.read(buf))
    }
}

fn _p_write_all(proc: &mut Session, buf: &[u8]) -> std::io::Result<()> {
    #[cfg(not(feature = "async"))]
    {
        proc.write_all(buf)
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.write_all(buf))
    }
}

fn _p_flush(proc: &mut Session) -> std::io::Result<()> {
    #[cfg(not(feature = "async"))]
    {
        proc.flush()
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.flush())
    }
}

fn _p_send(proc: &mut Session, buf: &str) -> std::io::Result<()> {
    #[cfg(not(feature = "async"))]
    {
        proc.send(buf)
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.send(buf))
    }
}

fn _p_expect(proc: &mut Session, n: impl Needle) -> Result<Captures, expectrl::Error> {
    #[cfg(not(feature = "async"))]
    {
        proc.expect(n)
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.expect(n))
    }
}

fn _p_send_line(proc: &mut Session, buf: &str) -> std::io::Result<()> {
    #[cfg(not(feature = "async"))]
    {
        proc.send_line(buf)
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.send_line(buf))
    }
}

fn _p_send_control(proc: &mut Session, buf: impl Into<ControlCode>) -> std::io::Result<()> {
    #[cfg(not(feature = "async"))]
    {
        proc.send_control(buf)
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.send_control(buf))
    }
}

fn _p_read_to_string(proc: &mut Session) -> std::io::Result<String> {
    let mut buf = String::new();
    #[cfg(not(feature = "async"))]
    {
        proc.read_to_string(&mut buf)?;
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.read_to_string(&mut buf))?;
    }
    Ok(buf)
}

fn _p_read_to_end(proc: &mut Session) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    #[cfg(not(feature = "async"))]
    {
        proc.read_to_end(&mut buf)?;
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.read_to_end(&mut buf))?;
    }
    Ok(buf)
}

fn _p_read_until(proc: &mut Session, ch: u8) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    #[cfg(not(feature = "async"))]
    {
        let n = proc.read_until(ch, &mut buf)?;
        buf = buf[..n].to_vec();
    }
    #[cfg(feature = "async")]
    {
        let n = block_on(proc.read_until(ch, &mut buf))?;
        buf = buf[..n].to_vec();
    }
    Ok(buf)
}

fn _p_read_line(proc: &mut Session) -> std::io::Result<String> {
    let mut buf = String::new();
    #[cfg(not(feature = "async"))]
    {
        proc.read_line(&mut buf)?;
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.read_line(&mut buf))?;
    }
    Ok(buf)
}

fn _p_is_empty(proc: &mut Session) -> std::io::Result<bool> {
    #[cfg(not(feature = "async"))]
    {
        proc.is_empty()
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.is_empty())
    }
}

fn _p_try_read(proc: &mut Session, buf: &mut [u8]) -> std::io::Result<usize> {
    #[cfg(not(feature = "async"))]
    {
        proc.try_read(buf)
    }
    #[cfg(feature = "async")]
    {
        block_on(async {
            futures_lite::future::poll_once(proc.read(buf))
                .await
                .unwrap_or(Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "")))
        })
    }
}

#[cfg(unix)]
fn _p_interact(proc: &mut Session) -> Result<(), expectrl::Error> {
    use expectrl::{interact::InteractOptions, stream::stdin::Stdin};
    use std::io::stdout;

    let mut stdin = Stdin::open()?;
    let stdout = stdout();

    #[cfg(not(feature = "async"))]
    {
        proc.interact(&mut stdin, stdout)
            .spawn(InteractOptions::default())?;
    }
    #[cfg(feature = "async")]
    {
        block_on(
            proc.interact(&mut stdin, stdout)
                .spawn(InteractOptions::default()),
        )?;
    }

    stdin.close()
}

#[cfg(windows)]
fn do_until(mut foo: impl FnMut() -> bool, timeout: Duration) -> bool {
    let now = std::time::Instant::now();
    while now.elapsed() < timeout {
        if foo() {
            return true;
        }
    }

    return false;
}
