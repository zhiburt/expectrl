use expectrl::{ControlCode, Session, WaitStatus};
use std::{process::Command, thread, time::Duration};

#[cfg(feature = "async")]
use futures_lite::{
    future::block_on,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt},
};

#[cfg(not(feature = "async"))]
use std::io::{BufRead, Read, Write};

#[test]
fn send_controll() {
    let mut proc = Session::spawn_cmd(Command::new("cat")).unwrap();
    _p_send_control(&mut proc, ControlCode::EOT).unwrap();
    assert_eq!(proc.wait().unwrap(), WaitStatus::Exited(proc.pid(), 0),);
}

#[test]
fn send() {
    let mut proc = Session::spawn_cmd(Command::new("cat")).unwrap();

    _p_send(&mut proc, "hello cat\n").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    let mut buf = vec![0; 128];
    let n = _p_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"hello cat\r\n");

    assert!(proc.exit(true).unwrap());
}

#[test]
fn send_line() {
    let mut proc = Session::spawn_cmd(Command::new("cat")).unwrap();

    _p_send_line(&mut proc, "hello cat").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    let mut buf = vec![0; 128];
    let n = _p_read(&mut proc, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"hello cat\r\n");

    assert!(proc.exit(true).unwrap());
}

#[test]
fn try_read_byte() {
    let mut proc = Session::spawn_cmd(Command::new("cat")).unwrap();

    assert_eq!(_p_try_read_byte(&mut proc).unwrap(), None);

    _p_send_line(&mut proc, "123").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    assert_eq!(_p_try_read_byte(&mut proc).unwrap(), Some(Some(b'1')));
    assert_eq!(_p_try_read_byte(&mut proc).unwrap(), Some(Some(b'2')));
    assert_eq!(_p_try_read_byte(&mut proc).unwrap(), Some(Some(b'3')));
    assert_eq!(_p_try_read_byte(&mut proc).unwrap(), Some(Some(b'\r')));
    assert_eq!(_p_try_read_byte(&mut proc).unwrap(), Some(Some(b'\n')));
    assert_eq!(_p_try_read_byte(&mut proc).unwrap(), None);
}

#[test]
fn blocking_read_after_non_blocking_try_read_byte() {
    let mut proc = Session::spawn_cmd(Command::new("cat")).unwrap();

    assert_eq!(_p_try_read_byte(&mut proc).unwrap(), None);

    _p_send_line(&mut proc, "123").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    assert_eq!(_p_try_read_byte(&mut proc).unwrap(), Some(Some(b'1')));

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
fn try_read() {
    let mut proc = Session::spawn_cmd(Command::new("cat")).unwrap();

    let mut buf = vec![0; 128];
    assert_eq!(_p_try_read(&mut proc, &mut buf).unwrap(), None);

    _p_send_line(&mut proc, "123").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    assert_eq!(_p_try_read(&mut proc, &mut buf).unwrap(), Some(5));
    assert_eq!(&buf[..5], b"123\r\n");
    assert_eq!(_p_try_read(&mut proc, &mut buf).unwrap(), None);
}

#[test]
fn blocking_read_after_non_blocking_try_read() {
    let mut proc = Session::spawn_cmd(Command::new("cat")).unwrap();

    let mut buf = vec![0; 1];
    assert_eq!(_p_try_read(&mut proc, &mut buf).unwrap(), None);

    _p_send_line(&mut proc, "123").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    assert_eq!(_p_try_read(&mut proc, &mut buf).unwrap(), Some(1));
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
fn try_read_after_eof() {
    let mut proc = Session::spawn_cmd(Command::new("cat")).unwrap();

    _p_send_line(&mut proc, "hello").unwrap();

    // give cat a time to react on input
    thread::sleep(Duration::from_millis(100));

    let mut buf = vec![0; 128];
    assert_eq!(_p_try_read(&mut proc, &mut buf).unwrap(), Some(7));
    assert_eq!(_p_try_read(&mut proc, &mut buf).unwrap(), None);
    assert_eq!(_p_try_read_byte(&mut proc).unwrap(), None);
}

#[test]
fn try_read_after_process_exit() {
    let mut command = Command::new("echo");
    command.arg("hello cat");
    let mut proc = Session::spawn_cmd(command).unwrap();

    // on macos we may not able to read after process is dead.
    // I assume that kernel consumes proceses resorces without any code check of parent,
    // which what is happening on linux.
    //
    // So we check that there may be None or Some(0)

    let mut buf = vec![0; 128];
    assert!(matches!(
        _p_try_read(&mut proc, &mut buf).unwrap(),
        Some(11) | None
    ));
    assert!(matches!(
        _p_try_read(&mut proc, &mut buf).unwrap(),
        Some(0) | None
    ));

    // on macos we can't put it before read's for some reason something get blocked
    // assert_eq!(proc.wait().unwrap(), WaitStatus::Exited(proc.pid(), 0));
}

#[test]
fn try_read_to_end() {
    let mut cmd = Command::new("echo");
    cmd.arg("Hello World");
    let mut proc = Session::spawn_cmd(cmd).unwrap();

    let mut buf = vec![0; 128];
    while _p_try_read(&mut proc, &mut buf).unwrap() != Some(0) {}

    assert_eq!(&buf[..13], b"Hello World\r\n");
}

#[test]
fn continues_try_reads() {
    let mut cmd = Command::new("python3");
    cmd.args(vec![
        "-c",
        "import time;\
        print('Start Sleep');\
        time.sleep(0.1);\
        print('End of Sleep');\
        yn=input('input');",
    ]);

    let mut proc = Session::spawn_cmd(cmd).unwrap();

    let mut buf = [0; 128];
    loop {
        if let Some(n) = _p_try_read(&mut proc, &mut buf).unwrap() {
            if String::from_utf8_lossy(&buf[..n]).contains("input") {
                break;
            }
        }
    }
}

#[test]
#[cfg(not(target_os = "macos"))]
fn automatic_stop_of_interact() {
    let mut p = Session::spawn_cmd(Command::new("ls")).unwrap();
    let status = _p_interact(&mut p).unwrap();

    // It may be finished not only because process is done but
    // also because it reached EOF.
    assert!(matches!(
        status,
        WaitStatus::Exited(_, 0) | WaitStatus::StillAlive
    ));

    // check that second spawn works
    let mut p = Session::spawn_cmd(Command::new("ls")).unwrap();
    let status = _p_interact(&mut p).unwrap();
    assert!(matches!(
        status,
        WaitStatus::Exited(_, 0) | WaitStatus::StillAlive
    ));
}

#[test]
#[cfg(not(target_os = "macos"))]
fn spawn_after_interact() {
    let mut p = Session::spawn_cmd(Command::new("ls")).unwrap();
    let _ = _p_interact(&mut p).unwrap();

    let p = Session::spawn_cmd(Command::new("ls")).unwrap();
    assert!(matches!(p.wait().unwrap(), WaitStatus::Exited(_, 0)));
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

fn _p_try_read_byte(proc: &mut Session) -> std::io::Result<Option<Option<u8>>> {
    #[cfg(not(feature = "async"))]
    {
        proc.try_read_byte()
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.try_read_byte())
    }
}

fn _p_try_read(proc: &mut Session, buf: &mut [u8]) -> std::io::Result<Option<usize>> {
    #[cfg(not(feature = "async"))]
    {
        proc.try_read(buf)
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.try_read(buf))
    }
}

fn _p_interact(proc: &mut Session) -> std::io::Result<WaitStatus> {
    #[cfg(not(feature = "async"))]
    {
        proc.interact()
    }
    #[cfg(feature = "async")]
    {
        block_on(proc.interact())
    }
}
