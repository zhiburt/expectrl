use expectrl::{repl::spawn_powershell, ControlCode, Regex};

#[cfg(windows)]
#[cfg(not(feature = "async"))]
fn main() {
    use std::io::Read;

    let mut p = spawn_powershell().unwrap();

    // case 1: execute
    let hostname = p.execute("hostname").unwrap();
    println!(
        "Current hostname: {:?}",
        String::from_utf8(hostname).unwrap()
    );

    // p.send_line("echo 123").unwrap();
    // let hostname = p.expect("NOT_HERE");
    
    // let mut buf = [0; 1024];
    // p.read(&mut buf).unwrap();
    // println!("____ {:?}", String::from_utf8_lossy(&buf));

    // p.send_line("[Console]::OutputEncoding = New-Object -typename System.Text.UTF8Encoding").unwrap();
    // let hostname = p.expect("NOT_HERE");

    // p.send_line("hostname").unwrap();
    // let hostname = p.expect("NOT_HERE");

    // p.send_line("hostname").unwrap();
    // let hostname = p.expect("NOT_HERE");
    
    // p.send_line("hostname").unwrap();
    // let hostname = p.expect("NOT_HERE");

    // p.send_line("hostname").unwrap();
    // let hostname = p.expect("NOT_HERE").unwrap();

    // let hostname = p.expect("NOT_HERE").unwrap();
    // println!(
    //     "Current hostname: {:?}",
    //     String::from_utf8(hostname.before_match().to_vec()).unwrap()
    // );

    // case 2: wait until done, only extract a few infos
    p.send_line("type README.md | Measure-Object -line -word -character").unwrap();
    let lines = p.expect(Regex("[0-9]+")).unwrap();
    let words = p.expect(Regex("[0-9]+")).unwrap();
    let bytes = p.expect(Regex("[0-9]+")).unwrap();
    // go sure `wc` is really done
    p.expect_prompt().unwrap();
    println!(
        "/etc/passwd has {} lines, {} words, {} chars",
        String::from_utf8_lossy(lines.found_match()),
        String::from_utf8_lossy(words.found_match()),
        String::from_utf8_lossy(bytes.found_match()),
    );

    // case 3: read while program is still executing
    p.send_line("ping 8.8.8.8 -t").unwrap();
    for _ in 0..5 {
        let duration = p.expect(Regex("[0-9.]+ms")).unwrap();
        println!(
            "Roundtrip time: {}",
            String::from_utf8_lossy(duration.found_match())
        );
    }
    p.send_control(ControlCode::ETX).unwrap();
    p.expect_prompt().unwrap();
}


#[cfg(not(windows))]
fn main() {
    panic!("An example doesn't supported on windows")
}
