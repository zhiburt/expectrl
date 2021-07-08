// An example is based on README.md from https://github.com/philippkeller/rexpect

use expectrl::{repl::spawn_bash, Regex};
use ptyprocess::ControlCode;
use std::io::BufRead;

fn main() {
    let mut p = spawn_bash().unwrap();

    // case 1: wait until program is done
    p.send_line("hostname").unwrap();
    let mut hostname = String::new();
    p.read_line(&mut hostname).unwrap();
    p.expect_prompt().unwrap(); // go sure `hostname` is really done
    println!("Current hostname: {:?}", hostname); // it prints some undetermined characters before hostname ...


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
