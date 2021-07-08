use expectrl::repl::{spawn_bash, spawn_python};
use ptyprocess::{ControlCode, WaitStatus};
use std::{io::BufRead, thread, time::Duration};

#[test]
fn bash() {
    let mut p = spawn_bash().unwrap();

    p.send_line("echo Hello World").unwrap();
    let mut msg = String::new();
    p.read_line(&mut msg).unwrap();
    assert!(msg.ends_with("Hello World\r\n"));

    thread::sleep(Duration::from_millis(300));
    p.send_control(ControlCode::EOT).unwrap();

    assert_eq!(p.wait().unwrap(), WaitStatus::Exited(p.pid(), 0));
}

#[test]
fn python() {
    let mut p = spawn_python().unwrap();

    p.execute("print('Hello World')").unwrap();
    let mut msg = String::new();
    p.read_line(&mut msg).unwrap();
    assert_eq!(msg, "Hello World\r\n");

    thread::sleep(Duration::from_millis(300));
    p.send_control(ControlCode::EndOfText).unwrap();
    thread::sleep(Duration::from_millis(300));

    let mut msg = String::new();
    p.read_line(&mut msg).unwrap();
    p.read_line(&mut msg).unwrap();
    assert_eq!(msg, ">>> \r\nKeyboardInterrupt\r\n");

    p.send_control(ControlCode::EndOfTransmission).unwrap();

    assert_eq!(p.wait().unwrap(), WaitStatus::Exited(p.pid(), 0));
}
