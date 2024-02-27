use expectrl::{
    interact::actions::lookup::Lookup, spawn, stream::stdin::Stdin, ControlCode, Error, Expect,
    Regex,
};
use std::io::stdout;

#[cfg(not(all(windows, feature = "polling")))]
#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    let mut p = spawn("ftp bks4-speedtest-1.tele2.net")?;

    let mut auth = false;
    let mut login_lookup = Lookup::new();
    let mut stdin = Stdin::open()?;

    p.interact(&mut stdin, stdout())
        .set_state(&mut auth)
        .on_output(move |ctx| {
            if login_lookup
                .on(ctx.buf, ctx.eof, "Login successful")?
                .is_some()
            {
                **ctx.state = true;
                return Ok(true);
            }

            Ok(false)
        })
        .spawn()?;

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
    Ok(())
}

#[cfg(any(all(windows, feature = "polling"), feature = "async"))]
fn main() {}
