#![cfg(feature = "async")]

use expectrl::repl::{spawn_bash, spawn_python};
use futures_lite::io::AsyncBufReadExt;
use ptyprocess::{ControlCode, WaitStatus};
use std::{thread, time::Duration};

#[test]
fn bash() {
    futures_lite::future::block_on(async {
        let mut p = spawn_bash().await.unwrap();

        p.send_line("echo Hello World").await.unwrap();
        let mut msg = String::new();
        p.read_line(&mut msg).await.unwrap();
        assert!(msg.ends_with("Hello World\r\n"));

        thread::sleep(Duration::from_millis(300));
        p.send_control(ControlCode::EOT).await.unwrap();

        assert_eq!(p.wait().unwrap(), WaitStatus::Exited(p.pid(), 0));
    })
}

#[test]
fn python() {
    let mut p = spawn_python().unwrap();

    futures_lite::future::block_on(async {
        p.execute("print('Hello World')").await.unwrap();
        let mut msg = String::new();
        p.read_line(&mut msg).await.unwrap();
        assert_eq!(msg, "Hello World\r\n");

        thread::sleep(Duration::from_millis(300));
        p.send_control(ControlCode::EndOfText).await.unwrap();
        thread::sleep(Duration::from_millis(300));

        let mut msg = String::new();
        p.read_line(&mut msg).await.unwrap();
        p.read_line(&mut msg).await.unwrap();
        thread::sleep(Duration::from_millis(300));
        assert_eq!(msg, ">>> \r\nKeyboardInterrupt\r\n");

        p.send_control(ControlCode::EndOfTransmission)
            .await
            .unwrap();

        assert_eq!(p.wait().unwrap(), WaitStatus::Exited(p.pid(), 0));
    })
}

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

#[test]
fn bash_control_chars() {
    futures_lite::future::block_on(async {
        let mut p = spawn_bash().await.unwrap();
        p.send_line("cat <(echo ready) -").await.unwrap();
        p.send_control(ControlCode::EndOfText).await.unwrap(); // abort: SIGINT
        p.expect_prompt().await.unwrap();
        p.send_line("cat <(echo ready) -").await.unwrap();
        thread::sleep(Duration::from_millis(100));
        p.send_control(ControlCode::Substitute).await.unwrap(); // suspend:SIGTSTPcon
        p.expect_prompt().await.unwrap();
    });
}
