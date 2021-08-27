/// To run an example run the following command
/// `cargo run --example interact`.
use expectrl::repl::spawn_bash;

#[cfg(not(feature = "async"))]
fn main() {
    let mut bash = spawn_bash().expect("Error while spawning bash");

    println!("Now you're in interacting mode");
    println!("To return control back to main type CTRL-]");

    let status = bash.interact().expect("Failed to start interact");

    println!("Quiting status {:?}", status);
}

#[cfg(feature = "async")]
fn main() {
    let mut bash = futures_lite::future::block_on(spawn_bash()).expect("Error while spawning bash");

    println!("Now you're in interacting mode");
    println!("To return control back to main type CTRL-]");

    let status = futures_lite::future::block_on(bash.interact()).expect("Failed to start interact");

    println!("Quiting status {:?}", status);
}
