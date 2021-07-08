use expectrl::{repl::spawn_python, Regex};

fn main() {
    let mut p = spawn_python().unwrap();

    p.execute("import platform").unwrap();
    p.send_line("platform.node()").unwrap();

    // todo: add support for matches in 'Found' + iterator?
    let found = p.expect(Regex(r"'\w+'")).unwrap();

    println!(
        "Platform {}",
        String::from_utf8_lossy(found.found_match())
    );
}
