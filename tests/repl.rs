use expectrl::{
    repl::{spawn_bash, spawn_python},
    Regex,
};
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

#[test]
fn bash_pwd() {
    let mut p = spawn_bash().unwrap();
    p.execute("cd /tmp/").unwrap();
    p.send_line("pwd").unwrap();
    let mut pwd = String::new();
    p.read_line(&mut pwd).unwrap();
    assert_eq!("/tmp\r\n", pwd);
}

#[test]
fn bash_control_chars() {
    let mut p = spawn_bash().unwrap();
    p.execute("cat <(echo ready) -").unwrap();
    p.send_control(ControlCode::EndOfText).unwrap(); // abort: SIGINT
    p.expect_prompt().unwrap();
    p.execute("cat <(echo ready) -").unwrap();
    p.send_control(ControlCode::Substitute).unwrap(); // suspend:SIGTSTPcon
    p.expect(Regex(r"(Stopped|suspended)\s+cat .*")).unwrap();
    p.send_line("fg").unwrap();
    p.execute("cat <(echo ready) -").unwrap();
    p.send_control(ControlCode::EndOfText).unwrap(); // abort: SIGINT
}
