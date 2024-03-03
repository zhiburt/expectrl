use expectrl::{repl::spawn_python, Expect, Regex};

#[cfg(feature = "async")]
use expectrl::AsyncExpect;

#[cfg(not(feature = "async"))]
fn main() {
    let mut p = spawn_python().unwrap();

    p.execute("import platform").unwrap();
    p.send_line("platform.node()").unwrap();

    let found = p.expect(Regex(r"'.*'")).unwrap();

    println!(
        "Platform {}",
        String::from_utf8_lossy(found.get(0).unwrap())
    );
}

#[cfg(feature = "async")]
fn main() {
    futures_lite::future::block_on(async {
        let mut p = spawn_python().await.unwrap();

        p.execute("import platform").await.unwrap();
        p.send_line("platform.node()").await.unwrap();

        let found = p.expect(Regex(r"'.*'")).await.unwrap();

        println!(
            "Platform {}",
            String::from_utf8_lossy(found.get(0).unwrap())
        );
    })
}
