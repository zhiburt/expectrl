use expectrl::{session::Session, ControlCode};
use std::{thread, time::Duration};

#[cfg(unix)]
use std::process::Command;

#[cfg(unix)]
use expectrl::WaitStatus;

#[cfg(windows)]
use expectrl::ProcAttr;

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
    let mut proc =
        Session::spawn(ProcAttr::cmd("powershell -C ping localhost".to_string())).unwrap();

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
    let mut proc =
        Session::spawn(ProcAttr::default().commandline("powershell -C type".to_string())).unwrap();
    thread::sleep(Duration::from_millis(1000));

    _p_send(&mut proc, "hello cat\r\n").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(1000));

    let mut buf = vec![0; 1024];
    let n = _p_read(&mut proc, &mut buf).unwrap();

    let s = String::from_utf8_lossy(&buf);
    if !s.contains("hello cat") {
        panic!(
            "Expected to get {:?} in the output, but got {:?}",
            "hello cat", s
        );
    }

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
    let mut proc = Session::spawn(ProcAttr::cmd("powershell -C type".to_string())).unwrap();

    thread::sleep(Duration::from_millis(1000));
    _p_send_line(&mut proc, "hello cat").unwrap();
    thread::sleep(Duration::from_millis(1000));

    let mut buf = vec![0; 1024];
    let n = _p_read(&mut proc, &mut buf).unwrap();
    let n = _p_read(&mut proc, &mut buf[n..]).unwrap();

    let s = String::from_utf8_lossy(&buf);
    if !s.contains("hello cat") {
        panic!(
            "Expected to get {:?} in the output, but got {:?}",
            "hello cat", s
        );
    }
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

    let mut proc =
        Session::spawn(ProcAttr::default().commandline("powershell".to_string())).unwrap();
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
    let mut proc =
        Session::spawn(ProcAttr::default().commandline("powershell".to_string())).unwrap();
    _p_send_line(
        &mut proc,
        "while (1) { read-host | set r; if (!$r) { break }}",
    )
    .unwrap();

    _p_send_line(&mut proc, "123").unwrap();

    thread::sleep(Duration::from_millis(100));

    let mut buf = [0; 1];
    _p_try_read(&mut proc, &mut buf).unwrap();
    println!("{:?}", String::from_utf8_lossy(&buf));
    assert_eq!(&buf, &[b'1']);

    let mut buf = [0; 64];
    let n = _p_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"23\r\n");
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
    let mut proc =
        Session::spawn(ProcAttr::default().commandline("powershell".to_string())).unwrap();
    _p_send_line(
        &mut proc,
        "while (1) { read-host | set r; if (!$r) { break }}",
    )
    .unwrap();
    thread::sleep(Duration::from_millis(1000));
    while !_p_try_read(&mut proc, &mut [0; 1]).is_err() {}

    assert_eq!(
        _p_try_read(&mut proc, &mut [0; 1]).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );

    _p_send_line(&mut proc, "123").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    let mut buf = vec![0; 128];
    assert_eq!(_p_try_read(&mut proc, &mut buf).unwrap(), 5);
    assert_eq!(&buf[..5], b"123\r\n");
    assert_eq!(
        _p_try_read(&mut proc, &mut buf).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
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

#[test]
#[cfg(windows)]
fn blocking_read_after_non_blocking_try_read() {
    let mut proc = Session::spawn(ProcAttr::cmd("powershell -C type".to_string())).unwrap();

    thread::sleep(Duration::from_millis(1000));
    while !_p_try_read(&mut proc, &mut [0; 1]).is_err() {}

    _p_send_line(&mut proc, "123").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(500));

    let mut buf = vec![0; 1024];
    _p_try_read(&mut proc, &mut buf).unwrap();

    let buf = String::from_utf8_lossy(&buf);

    if !buf.contains("123") {
        panic!(
            "Expected to get {:?} in the output, but got {:?}",
            "123", buf
        );
    }
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

#[test]
#[cfg(windows)]
fn try_read_after_process_exit() {
    let mut proc = Session::spawn(ProcAttr::cmd("echo hello cat".to_string())).unwrap();

    assert_eq!(proc.wait(None).unwrap(), 0);

    assert_eq!(_p_try_read(&mut proc, &mut [0; 128]).unwrap(), 59);
    assert!(_p_try_read(&mut proc, &mut [0; 128]).is_err());
    assert!(_p_try_read(&mut proc, &mut [0; 128]).is_err());
    assert!(_p_is_empty(&mut proc).unwrap());

    assert_eq!(proc.wait(None).unwrap(), 0);
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
    let mut proc = Session::spawn(ProcAttr::cmd("echo Hello World".to_string())).unwrap();

    thread::sleep(Duration::from_millis(1000));

    let mut v: Vec<u8> = Vec::new();
    let mut b = [0; 1];
    loop {
        match _p_try_read(&mut proc, &mut b) {
            Ok(n) => {
                v.extend(&b[..n]);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(err) => Err(err).unwrap(),
        }
    }

    assert!(String::from_utf8_lossy(&v).contains("Hello World"));
}

#[test]
#[cfg(windows)]
fn continues_try_reads() {
    let cmd = ProcAttr::default().commandline("python3 -c \"import time; print('Start Sleep'); time.sleep(0.1); print('End of Sleep'); yn=input('input');\"".to_string());

    let mut proc = Session::spawn(cmd).unwrap();

    let mut buf = [0; 128];
    loop {
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
fn automatic_stop_of_interact() {
    let mut p = Session::spawn(Command::new("ls")).unwrap();
    let status = _p_interact(&mut p).unwrap();

    // It may be finished not only because process is done but
    // also because it reached EOF.
    assert!(matches!(
        status,
        WaitStatus::Exited(_, 0) | WaitStatus::StillAlive
    ));

    // check that second spawn works
    let mut p = Session::spawn(Command::new("ls")).unwrap();
    let status = _p_interact(&mut p).unwrap();
    assert!(matches!(
        status,
        WaitStatus::Exited(_, 0) | WaitStatus::StillAlive
    ));
}

#[test]
#[cfg(not(target_os = "macos"))]
#[cfg(not(windows))]
fn spawn_after_interact() {
    let mut p = Session::spawn(Command::new("ls")).unwrap();
    let _ = _p_interact(&mut p).unwrap();

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
fn _p_interact(proc: &mut Session) -> Result<WaitStatus, expectrl::Error> {
    #[cfg(not(feature = "async"))]
    {
        proc.interact()
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.interact())
    }
}
