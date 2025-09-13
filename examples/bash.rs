// An example is based on README.md from https://github.com/philippkeller/rexpect

#[cfg(unix)]
use expectrl::{repl::spawn_bash, ControlCode, Expect, Regex};

#[cfg(unix)]
#[cfg(not(feature = "async"))]
fn main() {
    let mut p = spawn_bash().unwrap();

    // case 1: execute
    let hostname = p.execute("hostname").unwrap();
    println!("Current hostname: {:?}", String::from_utf8_lossy(&hostname));

    // case 2: wait until done, only extract a few infos
    p.send_line("wc /etc/passwd").unwrap();
    // `expect` returns both string-before-match and match itself, discard first
    let found = p.expect(Regex("([0-9]+).*([0-9]+).*([0-9]+)")).unwrap();
    let lines = String::from_utf8_lossy(&found[1]);
    let words = String::from_utf8_lossy(&found[2]);
    let chars = String::from_utf8_lossy(&found[3]);
    p.expect_prompt().unwrap(); // go sure `wc` is really done
    println!(
        "/etc/passwd has {} lines, {} words, {} chars",
        lines, words, chars,
    );

    // case 3: read while program is still executing
    p.send_line("ping 8.8.8.8").unwrap(); // returns when it sees "bytes of data" in output
    for _ in 0..5 {
        // times out if one ping takes longer than 2s
        let duration = p.expect(Regex("[0-9. ]+ ms")).unwrap();
        println!("Roundtrip time: {}", String::from_utf8_lossy(&duration[0]));
    }

    p.send(ControlCode::EOT).unwrap();
}

#[cfg(unix)]
#[cfg(feature = "async")]
fn main() {
    use expectrl::AsyncExpect;
    use futures_lite::io::AsyncBufReadExt;

    let f = async {
        let mut p = spawn_bash().await.unwrap();

        // case 1: wait until program is done
        p.send_line("hostname").await.unwrap();
        let mut hostname = String::new();
        p.read_line(&mut hostname).await.unwrap();
        p.expect_prompt().await.unwrap(); // go sure `hostname` is really done
        println!("Current hostname: {hostname:?}"); // it prints some undetermined characters before hostname ...

        // case 2: wait until done, only extract a few infos
        p.send_line("wc /etc/passwd").await.unwrap();
        // `expect` returns both string-before-match and match itself, discard first
        let found = p
            .expect(Regex("([0-9]+).*([0-9]+).*([0-9]+)"))
            .await
            .unwrap();
        let lines = String::from_utf8_lossy(&found[1]);
        let words = String::from_utf8_lossy(&found[2]);
        let chars = String::from_utf8_lossy(&found[3]);
        p.expect_prompt().await.unwrap(); // go sure `wc` is really done
        println!(
            "/etc/passwd has {} lines, {} words, {} chars",
            lines, words, chars,
        );

        // case 3: read while program is still executing
        p.send_line("ping 8.8.8.8").await.unwrap(); // returns when it sees "bytes of data" in output
        for _ in 0..5 {
            // times out if one ping takes longer than 2s
            let duration = p.expect(Regex("[0-9. ]+ ms")).await.unwrap();
            println!(
                "Roundtrip time: {}",
                String::from_utf8_lossy(duration.get(0).unwrap())
            );
        }

        p.send(ControlCode::EOT).await.unwrap();
    };

    futures_lite::future::block_on(f);
}

#[cfg(windows)]
fn main() {
    panic!("An example is not supported on windows")
}
