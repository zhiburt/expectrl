/// To run an example run the following command
/// `cargo run --example interact`.

#[cfg(unix)]
use expectrl::repl::spawn_bash;

#[cfg(unix)]
#[cfg(not(feature = "async"))]
fn main() {
    let mut bash = spawn_bash().expect("Error while spawning bash");

    println!("Now you're in interacting mode");
    println!("To return control back to main type CTRL-]");

    let status = bash.interact().expect("Failed to start interact");

    println!("Quiting status {:?}", status);
}

#[cfg(unix)]
#[cfg(feature = "async")]
fn main() {
    let mut bash = futures_lite::future::block_on(spawn_bash()).expect("Error while spawning bash");

    println!("Now you're in interacting mode");
    println!("To return control back to main type CTRL-]");

    let status = futures_lite::future::block_on(bash.interact()).expect("Failed to start interact");

    println!("Quiting status {:?}", status);
}

#[cfg(windows)]
#[cfg(not(feature = "async"))]
fn main() {
    let mut pwsh = expectrl::spawn("pwsh").expect("Error while spawning bash");

    println!("Now you're in interacting mode");
    println!("To return control back to main type CTRL-]");

    pwsh.interact().expect("Failed to start interact");

    println!("Quiting");
}

#[cfg(windows)]
#[cfg(feature = "async")]
fn main() {
    let mut pwsh = expectrl::spawn("pwsh").expect("Error while spawning bash");

    println!("Now you're in interacting mode");
    println!("To return control back to main type CTRL-]");

    futures_lite::future::block_on(pwsh.interact()).expect("Failed to start interact");

    println!("Quiting");
}
