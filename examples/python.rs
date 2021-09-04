use expectrl::{repl::spawn_python, Regex};

#[cfg(not(feature = "async"))]
fn main() {
    let mut p = spawn_python().unwrap();

    p.execute("import platform").unwrap();
    p.send_line("platform.node()").unwrap();

    // todo: add support for matches in 'Found' + iterator?
    let found = p.expect(Regex(r"'.*'")).unwrap();

    println!("Platform {}", String::from_utf8_lossy(found.first()));
}

#[cfg(feature = "async")]
fn main() {
    let mut p = spawn_python().unwrap();

    futures_lite::future::block_on(async {
        p.execute("import platform").await.unwrap();
        p.send_line("platform.node()").await.unwrap();

        // todo: add support for matches in 'Found' + iterator?
        let found = p.expect(Regex(r"'.*'")).await.unwrap();

        println!("Platform {}", String::from_utf8_lossy(found.first()));
    })
}
