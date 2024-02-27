use expectrl::{spawn, Error, Expect};

fn main() -> Result<(), Error> {
    let p = spawn("cat")?;
    let mut p = expectrl::session::log(p, std::io::stdout())?;

    #[cfg(not(feature = "async"))]
    {
        p.send_line("Hello World")?;
        p.expect("Hello World")?;
    }
    #[cfg(feature = "async")]
    {
        futures_lite::future::block_on(async {
            p.send_line("Hello World").await?;
            p.expect("Hello World").await
        })?;
    }

    Ok(())
}
