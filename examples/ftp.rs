use expectrl::{spawn, ControlCode, Error, Regex, WaitStatus};

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    let mut p = spawn("ftp bks4-speedtest-1.tele2.net")?;
    p.expect(Regex("Name \\(.*\\):"))?;
    p.send_line("anonymous")?;
    p.expect("Password")?;
    p.send_line("test")?;
    p.expect("ftp>")?;
    p.send_line("cd upload")?;
    p.expect("successfully changed.")?;
    p.send_line("pwd")?;
    p.expect(Regex("[0-9]+ \"/upload\""))?;
    p.send_control(ControlCode::EndOfTransmission)?;
    p.expect("Goodbye.")?;
    assert_eq!(p.wait(), Ok(WaitStatus::Exited(p.pid(), 0)));
    Ok(())
}

#[cfg(feature = "async")]
fn main() {}
