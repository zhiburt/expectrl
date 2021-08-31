//! Expectrl a library for running, controlling and communicating with a process.
//!
//! It supports `async/await`. To use it you should specify a `async` feature.
//!
//! # Example
//!
//! ```no_run,ignore
//! use expectrl::{spawn, Regex, Eof, WaitStatus};
//!
//! let mut p = spawn("ftp speedtest.tele2.net").unwrap();
//! p.expect(Regex("Name \\(.*\\):")).unwrap();
//! p.send_line("anonymous").unwrap();
//! p.expect("Password").unwrap();
//! p.send_line("test").unwrap();
//! p.expect("ftp>").unwrap();
//! p.send_line("cd upload").unwrap();
//! p.expect("successfully changed.\r\nftp>").unwrap();
//! p.send_line("pwd").unwrap();
//! p.expect(Regex("[0-9]+ \"/upload\"")).unwrap();
//! p.send_line("exit").unwrap();
//! p.expect(Eof).unwrap();
//! assert_eq!(p.wait().unwrap(), WaitStatus::Exited(p.pid(), 0));
//! ```

mod control_code;
mod error;
mod expect;
mod log;
#[cfg(unix)]
pub mod repl;
mod session;
mod stream;

use std::process::Command;

pub use control_code::ControlCode;
pub use error::Error;
pub use expect::{Any, Eof, NBytes, Needle, Regex};
pub use session::Found;

#[cfg(unix)]
pub use ptyprocess::{Signal, WaitStatus};

#[cfg(not(feature = "log"))]
pub type Session = session::Session;

#[cfg(feature = "log")]
pub type Session = log::SessionWithLog;

/// Spawn spawnes a new session.
///
/// It accepts a command and possibly arguments just as string.
/// It doesn't parses ENV variables. For complex constrictions use [`Session::spawn`].
///
/// # Example
///
/// ```no_run,ignore
/// use expectrl::{spawn, ControlCode};
/// use std::{thread, time::Duration};
/// use std::io::{Read, Write};
///
/// let mut p = spawn("cat").unwrap();
/// p.send_line("Hello World").unwrap();
///
/// thread::sleep(Duration::from_millis(300)); // give 'cat' some time to set up
/// p.send_control(ControlCode::EndOfText).unwrap(); // abort: SIGINT
///
/// let mut buf = String::new();
/// p.read_to_string(&mut buf).unwrap();
///
/// assert_eq!(buf, "Hello World\r\n");
/// ```
///
/// [`Session::spawn`]: ./struct.Session.html?#spawn
pub fn spawn<S: AsRef<str>>(cmd: S) -> Result<Session, Error> {
    #[cfg(unix)]
    {
        let args = tokenize_command(cmd.as_ref());
        if args.is_empty() {
            return Err(Error::CommandParsing);
        }
    
        let mut command = Command::new(&args[0]);
        command.args(args.iter().skip(1));
    
        Session::spawn(command)
    }
    #[cfg(windows)]
    {
        Session::spawn(conpty::ProcAttr::cmd(cmd.as_ref().to_owned()))
    }
}

/// Turn e.g. "prog arg1 arg2" into ["prog", "arg1", "arg2"]
/// It takes care of single and double quotes but,
///
/// It doesn't cover all edge cases.
/// So it may not be compatible with real shell arguments parsing.
fn tokenize_command(program: &str) -> Vec<String> {
    let re = regex::Regex::new(r#""[^"]+"|'[^']+'|[^'" ]+"#).unwrap();
    let mut res = vec![];
    for cap in re.captures_iter(program) {
        res.push(cap[0].to_string());
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_command() {
        let res = tokenize_command("prog arg1 arg2");
        assert_eq!(vec!["prog", "arg1", "arg2"], res);

        let res = tokenize_command("prog -k=v");
        assert_eq!(vec!["prog", "-k=v"], res);

        let res = tokenize_command("prog 'my text'");
        assert_eq!(vec!["prog", "'my text'"], res);

        let res = tokenize_command(r#"prog "my text""#);
        assert_eq!(vec!["prog", r#""my text""#], res);
    }

    #[cfg(unix)]
    #[test]
    fn test_spawn_no_command() {
        assert!(matches!(spawn(""), Err(Error::CommandParsing)));
    }

    #[test]
    #[ignore = "it's a compile time check"]
    fn session_as_writer() {
        #[cfg(not(feature = "async"))]
        {
            let _: Box<dyn std::io::Write> =
                Box::new(spawn("ls").unwrap()) as Box<dyn std::io::Write>;
            let _: Box<dyn std::io::Read> =
                Box::new(spawn("ls").unwrap()) as Box<dyn std::io::Read>;
            let _: Box<dyn std::io::BufRead> =
                Box::new(spawn("ls").unwrap()) as Box<dyn std::io::BufRead>;

            fn _io_copy(mut session: Session) {
                std::io::copy(&mut std::io::empty(), &mut session).unwrap();
            }
        }
        #[cfg(feature = "async")]
        {
            let _: Box<dyn futures_lite::AsyncWrite> =
                Box::new(spawn("ls").unwrap()) as Box<dyn futures_lite::AsyncWrite>;
            let _: Box<dyn futures_lite::AsyncRead> =
                Box::new(spawn("ls").unwrap()) as Box<dyn futures_lite::AsyncRead>;
            let _: Box<dyn futures_lite::AsyncBufRead> =
                Box::new(spawn("ls").unwrap()) as Box<dyn futures_lite::AsyncBufRead>;

            async fn _io_copy(mut session: Session) {
                futures_lite::io::copy(futures_lite::io::empty(), &mut session)
                    .await
                    .unwrap();
            }
        }
    }
}
