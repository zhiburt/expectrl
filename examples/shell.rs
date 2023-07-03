use expectrl::repl::ReplSession;
use std::io::Result;

#[cfg(all(unix, not(feature = "async")))]
fn main() -> Result<()> {
    let mut p = expectrl::spawn("sh")?;
    p.get_process_mut().set_echo(true, None)?;

    let mut shell = ReplSession::new(p, String::from("sh-5.1$"), Some(String::from("exit")), true);

    shell.expect_prompt()?;

    let output = exec(&mut shell, "echo Hello World")?;
    println!("{:?}", output);

    let output = exec(&mut shell, "echo '2 + 3' | bc")?;
    println!("{:?}", output);

    Ok(())
}

#[cfg(all(unix, not(feature = "async")))]
fn exec(shell: &mut ReplSession, cmd: &str) -> Result<String> {
    let buf = shell.execute(cmd)?;
    let mut string = String::from_utf8_lossy(&buf).into_owned();
    string = string.replace("\r\n\u{1b}[?2004l\r", "");
    string = string.replace("\r\n\u{1b}[?2004h", "");

    Ok(string)
}

#[cfg(all(unix, feature = "async"))]
fn main() -> Result<()>{
    futures_lite::future::block_on(async {
        let mut p = expectrl::spawn("sh")?;
        p.get_process_mut().set_echo(true, None)?;
    
        let mut shell = ReplSession::new(p, String::from("sh-5.1$"), Some(String::from("exit")), true);
    
        shell.expect_prompt().await?;
    
        let output = exec(&mut shell, "echo Hello World").await?;
        println!("{:?}", output);
    
        let output = exec(&mut shell, "echo '2 + 3' | bc").await?;
        println!("{:?}", output);

        Ok(())
    })
}

#[cfg(all(unix, feature = "async"))]
async fn exec(shell: &mut ReplSession, cmd: &str) -> Result<String> {
    let buf = shell.execute(cmd).await?;
    let mut string = String::from_utf8_lossy(&buf).into_owned();
    string = string.replace("\r\n\u{1b}[?2004l\r", "");
    string = string.replace("\r\n\u{1b}[?2004h", "");

    Ok(string)
}

#[cfg(windows)]
fn main() {
    panic!("An example doesn't supported on windows")
}
