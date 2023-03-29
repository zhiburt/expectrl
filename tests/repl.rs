#![cfg(unix)]

use expectrl::{
    repl::{spawn_bash, spawn_python},
    ControlCode, WaitStatus,
};
#[cfg(feature = "async")]
use futures_lite::io::AsyncBufReadExt;
#[cfg(not(feature = "async"))]
use std::io::BufRead;
use std::{thread, time::Duration};

#[cfg(not(feature = "async"))]
#[cfg(target_os = "linux")]
#[test]
fn bash() {
    let mut p = spawn_bash().unwrap();

    p.send_line("echo Hello World").unwrap();
    let mut msg = String::new();
    p.read_line(&mut msg).unwrap();
    assert!(msg.ends_with("Hello World\r\n"));

    p.send(ControlCode::EOT).unwrap();

    assert_eq!(
        p.get_process().wait().unwrap(),
        WaitStatus::Exited(p.get_process().pid(), 0)
    );
}

#[cfg(not(feature = "async"))]
#[cfg(target_os = "linux")]
#[test]
fn bash_with_log() {
    use expectrl::{repl::ReplSession, session};

    let p = spawn_bash().unwrap();
    let prompt = p.get_prompt().to_owned();
    let quit_cmd = p.get_quit_command().map(|c| c.to_owned());
    let is_echo = p.is_echo();
    let session = session::log(p.into_session(), std::io::stderr()).unwrap();
    let mut p = ReplSession::new(session, prompt, quit_cmd, is_echo);

    p.send_line("echo Hello World").unwrap();
    let mut msg = String::new();
    p.read_line(&mut msg).unwrap();
    assert!(msg.ends_with("Hello World\r\n"));

    thread::sleep(Duration::from_millis(300));
    p.send(ControlCode::EOT).unwrap();

    assert_eq!(
        p.get_process().wait().unwrap(),
        WaitStatus::Exited(p.get_process().pid(), 0)
    );
}

#[cfg(feature = "async")]
#[test]
fn bash() {
    futures_lite::future::block_on(async {
        let mut p = spawn_bash().await.unwrap();

        p.send_line("echo Hello World").await.unwrap();
        let mut msg = String::new();
        p.read_line(&mut msg).await.unwrap();
        assert!(msg.ends_with("Hello World\r\n"));

        thread::sleep(Duration::from_millis(300));
        p.send(ControlCode::EOT).await.unwrap();

        assert_eq!(p.wait().unwrap(), WaitStatus::Exited(p.pid(), 0));
    })
}

#[cfg(feature = "async")]
#[test]
fn bash_with_log() {
    futures_lite::future::block_on(async {
        use expectrl::{repl::ReplSession, session};

        let p = spawn_bash().await.unwrap();
        let prompt = p.get_prompt().to_owned();
        let quit_cmd = p.get_quit_command().map(|c| c.to_owned());
        let is_echo = p.is_echo();
        let session = session::log(p.into_session(), std::io::stderr()).unwrap();
        let mut p = ReplSession::new(session, prompt, quit_cmd, is_echo);

        p.send_line("echo Hello World").await.unwrap();
        let mut msg = String::new();
        p.read_line(&mut msg).await.unwrap();
        assert!(msg.ends_with("Hello World\r\n"));

        thread::sleep(Duration::from_millis(300));
        p.send(ControlCode::EOT).await.unwrap();

        assert_eq!(p.wait().unwrap(), WaitStatus::Exited(p.pid(), 0));
    })
}

#[cfg(not(feature = "async"))]
#[test]
fn python() {
    let mut p = spawn_python().unwrap();

    let prompt = p.execute("print('Hello World')").unwrap();
    let prompt = String::from_utf8_lossy(&prompt);
    assert!(prompt.contains("Hello World"), "{prompt:?}");

    thread::sleep(Duration::from_millis(300));
    p.send(ControlCode::EndOfText).unwrap();
    thread::sleep(Duration::from_millis(300));

    let mut msg = String::new();
    p.read_line(&mut msg).unwrap();
    assert!(msg.contains("\r\n"), "{msg:?}");

    let mut msg = String::new();
    p.read_line(&mut msg).unwrap();
    assert_eq!(msg, "KeyboardInterrupt\r\n");

    p.expect_prompt().unwrap();

    p.send(ControlCode::EndOfTransmission).unwrap();

    assert_eq!(
        p.get_process().wait().unwrap(),
        WaitStatus::Exited(p.get_process().pid(), 0)
    );
}

#[cfg(feature = "async")]
#[test]
fn python() {
    futures_lite::future::block_on(async {
        let mut p = spawn_python().await.unwrap();

        let prompt = p.execute("print('Hello World')").await.unwrap();
        let prompt = String::from_utf8_lossy(&prompt);
        assert!(prompt.contains("Hello World"), "{prompt:?}");

        thread::sleep(Duration::from_millis(300));
        p.send(ControlCode::EndOfText).await.unwrap();
        thread::sleep(Duration::from_millis(300));

        let mut msg = String::new();
        p.read_line(&mut msg).await.unwrap();
        assert!(msg.contains("\r\n"), "{msg:?}");

        let mut msg = String::new();
        p.read_line(&mut msg).await.unwrap();
        assert_eq!(msg, "KeyboardInterrupt\r\n");

        p.expect_prompt().await.unwrap();

        p.send(ControlCode::EndOfTransmission).await.unwrap();

        assert_eq!(p.wait().unwrap(), WaitStatus::Exited(p.pid(), 0));
    })
}

#[cfg(feature = "async")]
#[test]
fn bash_pwd() {
    futures_lite::future::block_on(async {
        let mut p = spawn_bash().await.unwrap();
        p.execute("cd /tmp/").await.unwrap();
        p.send_line("pwd").await.unwrap();
        let mut pwd = String::new();
        p.read_line(&mut pwd).await.unwrap();
        assert!(pwd.contains("/tmp\r\n"));
    });
}

#[cfg(feature = "async")]
#[test]
fn bash_control_chars() {
    futures_lite::future::block_on(async {
        let mut p = spawn_bash().await.unwrap();
        p.send_line("cat <(echo ready) -").await.unwrap();
        thread::sleep(Duration::from_millis(100));
        p.send(ControlCode::EndOfText).await.unwrap(); // abort: SIGINT
        p.expect_prompt().await.unwrap();
        p.send_line("cat <(echo ready) -").await.unwrap();
        thread::sleep(Duration::from_millis(100));
        p.send(ControlCode::Substitute).await.unwrap(); // suspend:SIGTSTPcon
        p.expect_prompt().await.unwrap();
    });
}

#[cfg(not(feature = "async"))]
#[test]
fn bash_pwd() {
    let mut p = spawn_bash().unwrap();
    p.execute("cd /tmp/").unwrap();
    p.send_line("pwd").unwrap();
    let mut pwd = String::new();
    p.read_line(&mut pwd).unwrap();
    assert!(pwd.contains("/tmp\r\n"));
}

#[cfg(not(feature = "async"))]
#[test]
fn bash_control_chars() {
    let mut p = spawn_bash().unwrap();
    p.send_line("cat <(echo ready) -").unwrap();
    thread::sleep(Duration::from_millis(300));
    p.send(ControlCode::EndOfText).unwrap(); // abort: SIGINT
    p.expect_prompt().unwrap();
    p.send_line("cat <(echo ready) -").unwrap();
    thread::sleep(Duration::from_millis(100));
    p.send(ControlCode::Substitute).unwrap(); // suspend:SIGTSTPcon
    p.expect_prompt().unwrap();
}
