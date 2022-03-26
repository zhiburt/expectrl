/// To run an example run the following command
/// `cargo run --example interact`.
use expectrl::spawn;

#[cfg(unix)]
const SHELL: &str = "sh";

#[cfg(windows)]
const SHELL: &str = "pwsh";

fn main() {
    let mut sh = spawn(SHELL).expect("Error while spawning sh");

    println!("Now you're in interacting mode");
    println!("To return control back to main type CTRL-] combination");

    #[cfg(not(feature = "async"))]
    sh.interact().expect("Failed to start interact");

    #[cfg(feature = "async")]
    futures_lite::future::block_on(sh.interact()).expect("Failed to start interact");
}
