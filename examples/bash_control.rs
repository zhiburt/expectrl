#[cfg(unix)]
use expectrl::{repl::spawn_bash, ControlCode, Error, Expect};

#[cfg(unix)]
#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    let mut p = spawn_bash()?;
    p.send_line("ping 8.8.8.8")?;
    p.expect("bytes of data")?;
    p.send_control(ControlCode::Substitute)?; // CTRL_Z
    p.expect_prompt()?;
    // bash writes 'ping 8.8.8.8' to stdout again to state which job was put into background
    p.send_line("bg")?;
    p.expect("ping 8.8.8.8")?;
    p.expect_prompt()?;
    p.send_line("sleep 0.5")?;
    p.expect_prompt()?;
    // bash writes 'ping 8.8.8.8' to stdout again to state which job was put into foreground
    p.send_line("fg")?;
    p.expect("ping 8.8.8.8")?;
    p.send_control(ControlCode::EndOfText)?;
    p.expect("packet loss")?;

    Ok(())
}

#[cfg(unix)]
#[cfg(feature = "async")]
fn main() -> Result<(), Error> {
    futures_lite::future::block_on(async {
        let mut p = spawn_bash().await?;
        p.send_line("ping 8.8.8.8").await?;
        p.expect("bytes of data").await?;
        p.send_control(ControlCode::Substitute).await?; // CTRL_Z
        p.expect_prompt().await?;
        // bash writes 'ping 8.8.8.8' to stdout again to state which job was put into background
        p.send_line("bg").await?;
        p.expect("ping 8.8.8.8").await?;
        p.expect_prompt().await?;
        p.send_line("sleep 0.5").await?;
        p.expect_prompt().await?;
        // bash writes 'ping 8.8.8.8' to stdout again to state which job was put into foreground
        p.send_line("fg").await?;
        p.expect("ping 8.8.8.8").await?;
        p.send_control(ControlCode::EndOfText).await?;
        p.expect("packet loss").await?;
        Ok(())
    })
}

#[cfg(windows)]
fn main() {
    panic!("An example doesn't supported on windows")
}
