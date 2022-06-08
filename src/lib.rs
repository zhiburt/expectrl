#![warn(
    missing_docs,
    future_incompatible,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_results,
    unused_variables,
    variant_size_differences
)]

//! # A tool for automating terminal applications on alike original expect.
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
//! ## Feature flags
//! 
//! - `async`: Enables a async/await public API.
//!
//! By default we generate documentation for interface with no features on.
//! You can generate documentation for other features localy using this command.
//! 
//! ```bash
//! cargo doc --features async --open
//! # or
//! RUSTDOCFLAGS='--cfg docsrs' cargo +nightly doc --features async --open
//! ```
//! 
//! ## Examples
//!
//! ### An example for interacting via ftp.
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
//! ### An example when `Command` is used.
//!
//! ```no_run,ignore
//! use std::{process::Command, io::prelude::*};
//! use expectrl::Session;
//!
//! let mut echo_hello = Command::new("sh");
//! echo_hello.arg("-c").arg("echo hello");
//!
//! let mut p = Session::spawn(echo_hello).unwrap();
//! p.expect("hello").unwrap();
//! ```
//!
//! ### An example of logging.
//!
//! ```no_run,ignore
//! use std::io::{stdout, prelude::*};
//! use expectrl::spawn;
//!
//! let mut sh = spawn("sh")
//!     .unwrap()
//!     .with_log(stdout())
//!     .unwrap();
//!
//! writeln!(sh, "Hello World").unwrap();
//! ```
//!
//! ### An example of `async` feature.
//!
//! You need to provide a `features=["async"]` flag to use it.
//!
//! ```no_run,ignore
//! use expectrl::spawn;
//!
//! let mut p = spawn("cat").await.unwrap();
//! p.expect("hello").await.unwrap();
//! ```
//!
//! [For more examples, check the examples directory.](https://github.com/zhiburt/expectrl/tree/main/examples)

mod captures;
mod check_macros;
mod control_code;
mod error;
mod needle;

pub mod interact;
pub mod process;
pub mod repl;
pub mod session;
pub mod stream;

pub use captures::Captures;
pub use control_code::ControlCode;
pub use error::Error;
pub use needle::{Any, Eof, NBytes, Needle, Regex};

#[cfg(unix)]
pub use ptyprocess::{Signal, WaitStatus};

pub use session::Session;

/// Spawn spawnes a new session.
///
/// It accepts a command and possibly arguments just as string.
/// It doesn't parses ENV variables. For complex constrictions use [`Session::spawn`].
///
/// # Example
///
/// ```no_run,ignore
/// use std::{thread, time::Duration, io::{Read, Write}};
/// use expectrl::{spawn, ControlCode};
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
    Session::spawn_cmd(cmd.as_ref())
}
