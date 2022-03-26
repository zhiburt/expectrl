use expectrl::{spawn, Error};

fn main() -> Result<(), Error> {
    let mut p = spawn("cat")?.with_log(std::io::stdout())?;

    #[cfg(not(feature = "async"))]
    {
        p.send_line("Hello World")?;
        p.expect("Hello World")?;
    }
    #[cfg(feature = "async")]
    {
        use futures_lite::future::block_on;
        block_on(p.send_line("Hello World"))?;
        block_on(p.expect("Hello World"))?;
    }

    Ok(())
}
