//! # A tool for automating terminal applications on Unix and on Windows.
//!
//! Using the library you can:
//!
//! - Spawn process
//! - Control process
//! - Interact with process's IO(input/output).
//!
//! `expectrl` like original `expect` may shine when you're working with interactive applications.
//! If your application is not interactive you may not find the library the best choise.
//!
//! ## Example
//!
//! An example for interacting via ftp.
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
//!
//! *The example inspired by the one in [philippkeller/rexpect].*
//!
//! [For more examples, check the examples directory.](https://github.com/zhiburt/expectrl/tree/main/examples)
//!
//! ## Features
//!
//! - It has an `async` support (To enable them you must turn on an `async` feature).
//! - It supports logging.
//! - It supports interact function.
//! - It has a Windows support.

#[cfg(unix)]
mod check_macros;
mod control_code;
mod error;
mod expect;
mod log;
mod process;
mod stream;

pub mod interact;
pub mod repl;
pub mod session;

pub use control_code::ControlCode;
pub use error::Error;
pub use expect::{Any, Eof, NBytes, Needle, Regex};
pub use process::Stream;
pub use session::Found;

#[cfg(windows)]
pub use conpty::ProcAttr;

#[cfg(unix)]
pub use ptyprocess::{Signal, WaitStatus};

use process::Process as ProcessTrait;
use std::{
    convert::TryInto,
    io::{self, BufRead, Read, Write},
};

pub trait Expect: Write + Read + BufRead {
    /// Expect waits until a pattern is matched.
    ///
    /// If the method returns [Ok] it is guaranteed that at least 1 match was found.
    ///
    /// This make assertions in a lazy manner.
    /// Starts from 1st byte then checks 2nd byte and goes further.
    /// It is done intentinally to be presize.
    /// It matters for example when you call this method with `crate::Regex("\\d+")` and output contains 123,
    /// expect will return '1' as a match not '123'.
    ///
    /// ```
    /// use expectrl::Expect;
    /// let mut p = expectrl::spawn("echo 123").unwrap();
    /// let m = p.expect(expectrl::Regex("\\d+")).unwrap();
    /// assert_eq!(m.first(), b"1");
    /// ```
    ///
    /// This behaviour is different from [Session::check].
    ///
    /// It return an error if timeout is reached.
    /// You can specify a timeout value by [Session::set_expect_timeout] method.
    fn expect<E: Needle>(&mut self, expect: E) -> Result<Found, Error>;

    /// Check checks if a pattern is matched.
    /// Returns empty found structure if nothing found.
    ///
    /// Is a non blocking version of [Session::expect].
    /// But its strategy of matching is different from it.
    /// It makes search agains all bytes available.
    ///
    /// ```
    /// use expectrl::Expect;
    /// let mut p = expectrl::spawn("echo 123").unwrap();
    /// // wait to guarantee that check will successed (most likely)
    /// std::thread::sleep(std::time::Duration::from_secs(1));
    /// let m = p.check(expectrl::Regex("\\d+")).unwrap();
    /// assert_eq!(m.first(), b"123");
    /// ```
    fn check<E: Needle>(&mut self, needle: E) -> Result<Found, Error>;

    /// Is matched checks if a pattern is matched.
    /// It doesn't consumes bytes from stream.
    ///
    /// Its strategy of matching is different from the one in [Session::expect].
    /// It makes search agains all bytes available.
    ///
    /// If you want to get a matched result [Session::check] and [Session::expect] is a better option,
    /// Because it is not guaranteed that [Session::check] or [Session::expect]
    /// with the same parameters:
    ///  * will successed even right after [Session::is_matched] call.
    ///  * will operate on the same bytes
    ///
    /// IMPORTANT:
    ///
    /// If you call this method with Eof pattern be aware that
    /// eof indication MAY be lost on the next interactions.
    /// It depends from a process you spawn.
    /// So it might be better to use [Session::check] or [Session::expect] with Eof.
    ///
    /// ```
    /// use expectrl::Expect;
    /// let mut p = expectrl::spawn("echo 123").unwrap();
    /// // wait to guarantee that check will successed (most likely)
    /// std::thread::sleep(std::time::Duration::from_secs(1));
    /// let m = p.is_matched(expectrl::Regex("\\d+")).unwrap();
    /// assert_eq!(m, true);
    /// ```
    fn is_matched<E: Needle>(&mut self, needle: E) -> Result<bool, Error>;

    /// Send text to child's `STDIN`.
    ///
    /// To write bytes you can use a [std::io::Write] operations instead.
    fn send(&mut self, s: impl AsRef<str>) -> io::Result<()>;

    /// Send a line to child's `STDIN`.
    fn send_line(&mut self, s: impl AsRef<str>) -> io::Result<()>;

    /// Send controll character to a child process.
    ///
    /// You must be carefull passing a char or &str as an argument.
    /// If you pass an unexpected controll you'll get a error.
    /// So it may be better to use [ControlCode].
    ///
    /// ```no_run
    /// use expectrl::{PtySession, ControlCode, Expect};
    /// use std::process::Command;
    ///
    /// #[cfg(unix)]
    /// let cmd = Command::new("cat");
    /// #[cfg(windows)]
    /// let cmd = expectrl::ProcAttr::cmd("cat".to_string());
    /// let mut process = PtySession::spawn_command(cmd).unwrap();
    /// process.send_control(ControlCode::EndOfText); // sends CTRL^C
    /// process.send_control('C'); // sends CTRL^C
    /// process.send_control("^C"); // sends CTRL^C
    /// ```
    fn send_control(&mut self, code: impl TryInto<ControlCode>) -> io::Result<()>;
}

impl<EE: Expect> Expect for &mut EE {
    fn expect<E: Needle>(&mut self, needle: E) -> Result<Found, Error> {
        (*self).expect(needle)
    }

    fn check<E: Needle>(&mut self, needle: E) -> Result<Found, Error> {
        (*self).check(needle)
    }

    fn is_matched<E: Needle>(&mut self, needle: E) -> Result<bool, Error> {
        (*self).is_matched(needle)
    }

    fn send(&mut self, s: impl AsRef<str>) -> io::Result<()> {
        (*self).send(s)
    }

    fn send_line(&mut self, s: impl AsRef<str>) -> io::Result<()> {
        (*self).send_line(s)
    }

    fn send_control(&mut self, code: impl TryInto<ControlCode>) -> io::Result<()> {
        (*self).send_control(code)
    }
}

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
pub fn spawn(cmd: impl AsRef<str>) -> Result<PtySession, Error> {
    let proc = PlatformProcess::spawn(cmd)?;
    let session = session::Session::from_process(proc)?;
    Ok(session)
}

#[cfg(unix)]
pub type PlatformProcess = process::unix::UnixProcess;

#[cfg(windows)]
pub type PlatformProcess = process::windows::WindowsProcess;

pub type PtySession = session::Session<PlatformProcess, <PlatformProcess as ProcessTrait>::Stream>;

impl PtySession {
    #[cfg(unix)]
    pub fn spawn_command(command: std::process::Command) -> Result<PtySession, Error> {
        let process = PlatformProcess::spawn_command(command)?;
        let session = PtySession::from_process(process)?;
        Ok(session)
    }

    #[cfg(windows)]
    pub fn spawn_command(attr: ProcAttr) -> Result<PtySession, Error> {
        let process = PlatformProcess::spawn_command(attr)?;
        let session = PtySession::from_process(process)?;
        Ok(session)
    }
}

impl<S: Stream> session::Session<PlatformProcess, S> {
    /// Interact gives control of the child process to the interactive user (the
    /// human at the keyboard).
    ///
    /// Returns a status of a process ater interactions.
    /// Why it's crusial to return a status is after check of is_alive the actuall
    /// status might be gone.
    ///
    /// Keystrokes are sent to the child process, and
    /// the `stdout` and `stderr` output of the child process is printed.
    ///
    /// When the user types the `escape_character` this method will return control to a running process.
    /// The escape_character will not be transmitted.
    /// The default for escape_character is entered as `Ctrl-]`, the very same as BSD telnet.
    ///
    /// This simply echos the child `stdout` and `stderr` to the real `stdout` and
    /// it echos the real `stdin` to the child `stdin`.
    #[cfg(unix)]
    #[cfg(not(feature = "async"))]
    pub fn interact(&mut self) -> Result<crate::WaitStatus, Error> {
        crate::interact::InteractOptions::terminal()?.interact(self)
    }

    /// Interact gives control of the child process to the interactive user (the
    /// human at the keyboard).
    ///
    /// Returns a status of a process ater interactions.
    /// Why it's crusial to return a status is after check of is_alive the actuall
    /// status might be gone.
    ///
    /// Keystrokes are sent to the child process, and
    /// the `stdout` and `stderr` output of the child process is printed.
    ///
    /// When the user types the `escape_character` this method will return control to a running process.
    /// The escape_character will not be transmitted.
    /// The default for escape_character is entered as `Ctrl-]`, the very same as BSD telnet.
    ///
    /// This simply echos the child `stdout` and `stderr` to the real `stdout` and
    /// it echos the real `stdin` to the child `stdin`.
    #[cfg(unix)]
    #[cfg(feature = "async")]
    pub async fn interact(&mut self) -> Result<WaitStatus, Error> {
        crate::interact::InteractOptions::terminal()?
            .interact(self)
            .await
    }

    /// Interact gives control of the child process to the interactive user (the
    /// human at the keyboard).
    #[cfg(windows)]
    pub fn interact(&mut self) -> Result<(), Error> {
        crate::interact::InteractOptions::terminal()?.interact(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn test_spawn_no_command() {
        assert!(
            matches!(spawn(""), Err(Error::IO(err)) if err.kind() == io::ErrorKind::InvalidInput && err.to_string() == "a commandline argument is not correct")
        );
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

            fn _io_copy(mut session: session::Session<impl ProcessTrait, impl Stream>) {
                std::io::copy(&mut std::io::empty(), &mut session).unwrap();
            }

            fn __io_copy(mut session: impl Expect) {
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

            async fn _io_copy(mut session: session::Session<impl ProcessTrait, impl Stream>) {
                futures_lite::io::copy(futures_lite::io::empty(), &mut session)
                    .await
                    .unwrap();
            }

            async fn __io_copy(mut session: impl Expect) {
                futures_lite::io::copy(futures_lite::io::empty(), &mut session)
                    .await
                    .unwrap();
            }
        }
    }
}
