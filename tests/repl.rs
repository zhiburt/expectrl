use std::{
    io::{BufRead, Read},
    thread,
    time::Duration,
};

use expectrl::{
    repl::{spawn_bash, spawn_python},
    Regex,
};
use ptyprocess::ControlCode;

// A test is based on README.md from https://github.com/philippkeller/rexpect
#[test]
fn bash() {
    let mut p = spawn_bash().unwrap();

    // case 1: wait until program is done
    p.send_line("hostname").unwrap();
    let mut hostname = String::new();
    p.read_line(&mut hostname).unwrap();
    p.expect_prompt().unwrap(); // go sure `hostname` is really done
    println!("Current hostname: {}", hostname);

    // case 2: wait until done, only extract a few infos
    p.send_line("wc /etc/passwd").unwrap();
    // `exp_regex` returns both string-before-match and match itself, discard first
    let lines = p.expect(Regex("[0-9]+")).unwrap();
    let words = p.expect(Regex("[0-9]+")).unwrap();
    let bytes = p.expect(Regex("[0-9]+")).unwrap();
    p.expect_prompt().unwrap(); // go sure `wc` is really done
    println!(
        "/etc/passwd has {} lines, {} words, {} chars",
        String::from_utf8_lossy(lines.found_match()),
        String::from_utf8_lossy(words.found_match()),
        String::from_utf8_lossy(bytes.found_match()),
    );

    // case 3: read while program is still executing
    p.send_line("ping 8.8.8.8").unwrap(); // returns when it sees "bytes of data" in output
    for _ in 0..5 {
        // times out if one ping takes longer than 2s
        let duration = p.expect(Regex("[0-9. ]+ ms")).unwrap();
        println!(
            "Roundtrip time: {}",
            String::from_utf8_lossy(duration.found_match())
        );
    }
    p.send_control(ControlCode::EOT).unwrap();
}

#[test]
fn python() {
    let mut p = spawn_python().unwrap();

    p.execute("import platform").unwrap();
    p.send_line("platform.node()").unwrap();

    let mut platform = String::new();
    p.read_line(&mut platform).unwrap();

    println!("Platform {}", platform);
}
