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
pub mod repl;
mod session;
mod stream;

pub use control_code::ControlCode;
pub use error::Error;
pub use expect::{Any, Eof, NBytes, Needle, Regex};
pub use ptyprocess::{Signal, WaitStatus};
pub use session::Found;

#[cfg(not(feature = "log"))]
pub type Session = session::Session;

#[cfg(feature = "log")]
pub type Session = log::SessionWithLog;

/// Spawn spawnes a new session.
///
/// It accepts a command and possibly arguments just as string.
/// It doesn't parses ENV variables. For complex constrictions use [`Session::spawn_cmd`].
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
/// [`Session::spawn_cmd`]: ./struct.Session.html?#spawn_cmd
pub fn spawn<S: AsRef<str>>(cmd: S) -> Result<Session, Error> {
    Session::spawn(cmd.as_ref())
}
