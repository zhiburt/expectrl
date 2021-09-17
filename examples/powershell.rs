use expectrl::{repl::spawn_powershell, ControlCode, Regex};

#[cfg(windows)]
fn main() {
    let mut p = spawn_powershell().unwrap();

    // case 1: execute
    let hostname = p.execute("hostname").unwrap();
    println!(
        "Current hostname: {:?}",
        String::from_utf8(hostname).unwrap()
    );

    // case 2: wait until done, only extract a few infos
    p.send_line("type README.md | Measure-Object -line -word -character")
        .unwrap();
    let lines = p.expect(Regex("[0-9]+\\s")).unwrap();
    let words = p.expect(Regex("[0-9]+\\s")).unwrap();
    let bytes = p.expect(Regex("([0-9]+)[^0-9]")).unwrap();
    // go sure `wc` is really done
    p.expect_prompt().unwrap();
    println!(
        "/etc/passwd has {} lines, {} words, {} chars",
        String::from_utf8_lossy(lines.first()),
        String::from_utf8_lossy(words.first()),
        String::from_utf8_lossy(bytes.matches()[1]),
    );

    // case 3: read while program is still executing
    p.send_line("ping 8.8.8.8 -t").unwrap();
    for _ in 0..5 {
        let duration = p.expect(Regex("[0-9.]+ms")).unwrap();
        println!(
            "Roundtrip time: {}",
            String::from_utf8_lossy(duration.first())
        );
    }

    p.send_control(ControlCode::ETX).unwrap();
    p.expect_prompt().unwrap();
}

#[cfg(not(windows))]
#[cfg(not(feature = "async"))]
fn main() {

    let mut p = spawn_powershell().unwrap();

    use std::io::Write;

    // case 1: execute
    let hostname = p.write_all(b"hostname\r").unwrap();
    p.write_all("\u{1b}[11;68R".as_bytes()).unwrap();
    let hostname = p.expect("EXPECTED_PROMPT>");
    let hostname = p.expect("EXPECTED_PROMPT>").unwrap().before().to_vec();
    println!(
        "Current hostname: {:?}",
        String::from_utf8(hostname).unwrap()
    );

    // case 2: wait until done, only extract a few infos
    p.send_line("wc /etc/passwd").unwrap();
    // `exp_regex` returns both string-before-match and match itself, discard first
    let lines = p.expect(Regex("[0-9]+")).unwrap();
    let words = p.expect(Regex("[0-9]+")).unwrap();
    let bytes = p.expect(Regex("[0-9]+")).unwrap();
    p.expect_prompt().unwrap(); // go sure `wc` is really done
    println!(
        "/etc/passwd has {} lines, {} words, {} chars",
        String::from_utf8_lossy(lines.first()),
        String::from_utf8_lossy(words.first()),
        String::from_utf8_lossy(bytes.first()),
    );

    // case 3: read while program is still executing
    p.send_line("ping 8.8.8.8").unwrap(); // returns when it sees "bytes of data" in output
    for _ in 0..5 {
        // times out if one ping takes longer than 2s
        let duration = p.expect(Regex("[0-9. ]+ ms")).unwrap();
        println!(
            "Roundtrip time: {}",
            String::from_utf8_lossy(duration.first())
        );
    }

    p.send_control(ControlCode::EOT).unwrap();
}

#[cfg(not(windows))]
#[cfg(feature = "async")]
fn main() {
    panic!("An example doesn't supported on windows")
}
