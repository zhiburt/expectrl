//! This module contains a routines for running and utilizing an interacting session with a [`Session`].
//!
#![cfg_attr(all(unix, not(feature = "async")), doc = "```no_run")]
#![cfg_attr(not(all(unix, not(feature = "async"))), doc = "```ignore")]
//! use expectrl::{interact::{InteractOptions, actions::lookup::Lookup}, spawn, stream::stdin::Stdin, Regex};
//!
//! #[derive(Debug)]
//! enum Answer {
//!     Yes,
//!     No,
//!     Unrecognized,
//! }
//!
//! let mut session = spawn("cat").expect("Can't spawn a session");
//!
//! let mut input_action = Lookup::new();
//!
//! let mut stdin = Stdin::open().unwrap();
//! let stdout = std::io::stdout();
//!
//! let mut opts = InteractOptions::new(Answer::Unrecognized)
//!     .on_input(|mut ctx| {
//!         let m = input_action.on(ctx.buf, ctx.eof, "yes")?;
//!         if m.is_some() {
//!             *ctx.state = Answer::Yes;
//!         };
//!
//!         let m = input_action.on(ctx.buf, ctx.eof, "no")?;
//!         if m.is_some() {
//!             *ctx.state = Answer::No;
//!         };
//!
//!         Ok(false)
//!     });
//!
//! session.interact(&mut stdin, stdout)
//!     .spawn(&mut opts)
//!     .expect("Failed to run an interact session");
//!
//! let answer = opts.into_inner();
//!
//! stdin.close().unwrap();
//!
//! println!("It was said {:?}", answer);
//! ```
//!
//! [`Session`]: crate::session::Session

pub mod actions;
mod context;
mod opts;
mod session;

pub use context::Context;
pub use opts::{InteractOptions, NoAction, NoFilter};
pub use session::InteractSession;
