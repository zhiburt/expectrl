use expectrl::{
    interact::{actions::lookup::Lookup, InteractOptions},
    spawn,
    stream::stdin::Stdin,
    ControlCode, Error, Regex, WaitStatus,
};
use std::io::stdout;

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    let mut auth = false;
    let mut login_lookup = Lookup::new();
    let opts = InteractOptions::new(&mut auth).on_output(|ctx| {
        if login_lookup
            .on(ctx.buf, ctx.eof, "Login successful")?
            .is_some()
        {
            **ctx.state = true;
            return Ok(true);
        }

        Ok(false)
    });

    let mut p = spawn("ftp bks4-speedtest-1.tele2.net")?;

    let mut stdin = Stdin::open()?;
    p.interact(&mut stdin, stdout()).spawn(opts)?;
    stdin.close()?;

    if !auth {
        println!("An authefication was not passed");
        return Ok(());
    }

    p.expect("ftp>")?;
    p.send_line("cd upload")?;
    p.expect("successfully changed.")?;
    p.send_line("pwd")?;
    p.expect(Regex("[0-9]+ \"/upload\""))?;
    p.send(ControlCode::EndOfTransmission)?;
    p.expect("Goodbye.")?;
    assert_eq!(p.wait(), Ok(WaitStatus::Exited(p.pid(), 0)));
    Ok(())
}

#[cfg(feature = "async")]
fn main() {}
