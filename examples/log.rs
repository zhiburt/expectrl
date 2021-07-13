use expectrl::Session;

#[cfg(feature = "sync")]
#[cfg(feature = "log")]
fn main() -> Result<(), expectrl::Error> {
    let mut p = Session::spawn("cat")?;
    p.set_writer(std::io::stdout());
    p.send_line("Hello World")?;

    // reading doesn't apear here because
    // under the hood we use buffering and buffering and we are not able to see this buffer.
    p.expect("Hello World")?;

    Ok(())
}

#[cfg(not(feature = "log"))]
fn main() {
    println!(
        "To run the example set necessary features `--no-default-features --features log,sync`"
    );

    std::process::exit(-1);
}
