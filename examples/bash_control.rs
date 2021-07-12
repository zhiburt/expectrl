#![cfg(feature = "sync")]

// An example is based on README.md from https://github.com/philippkeller/rexpect

use expectrl::{repl::spawn_bash, Error};
use ptyprocess::ControlCode;

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
