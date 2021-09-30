#[cfg(not(feature = "async"))]
fn main() {
    let mut session =
        expectrl::spawn("python ./tests/source/ansi.py").expect("Can't spawn a session");

    let mut opts = expectrl::interact::InteractOptions::terminal()
        .unwrap()
        .on_input("HelloWorld!", |_, _| {
            print!("You typed a magic word...\r\n");
            Ok(())
        });

    opts.interact(&mut session).unwrap();
}

#[cfg(feature = "async")]
fn main() {
    let mut session =
        expectrl::spawn("python ./tests/source/ansi.py").expect("Can't spawn a session");

    let opts = expectrl::interact::InteractOptions::terminal()
        .unwrap()
        .on_input("HelloWorld!", |_, _| {
            print!("You typed a magic word...\r\n");
            Ok(())
        });

    futures_lite::future::block_on(async {
        opts.interact(&mut session).await.unwrap();
    });
}
