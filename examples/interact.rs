//! To run an example run `cargo run --example interact`.

use expectrl::{spawn, stream::stdin::Stdin};
use std::io::stdout;

#[cfg(unix)]
const SHELL: &str = "sh";

#[cfg(windows)]
const SHELL: &str = "powershell";

#[cfg(not(all(windows, feature = "polling")))]
#[cfg(not(feature = "async"))]
fn main() {
    let mut sh = spawn(SHELL).expect("Error while spawning sh");

    println!("Now you're in interacting mode");
    println!("To return control back to main type CTRL-] combination");

    let mut stdin = Stdin::open().expect("Failed to create stdin");

    sh.interact(&mut stdin, stdout())
        .spawn()
        .expect("Failed to start interact");

    stdin.close().expect("Failed to close a stdin");

    println!("Exiting");
}

#[cfg(feature = "async")]
fn main() {
    // futures_lite::future::block_on(async {
    //     let mut sh = spawn(SHELL).expect("Error while spawning sh");

    //     println!("Now you're in interacting mode");
    //     println!("To return control back to main type CTRL-] combination");

    //     let mut stdin = Stdin::open().expect("Failed to create stdin");

    //     sh.interact(&mut stdin, stdout())
    //         .spawn()
    //         .await
    //         .expect("Failed to start interact");

    //     stdin.close().expect("Failed to close a stdin");

    //     println!("Exiting");
    // });
}

#[cfg(all(windows, feature = "polling", not(feature = "async")))]
fn main() {}
