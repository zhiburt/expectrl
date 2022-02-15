#[cfg(feature = "async")]
mod async_session;
#[cfg(feature = "async")]
mod async_stream;
#[cfg(not(feature = "async"))]
mod sync_session;
pub mod sync_stream;

use crate::{stream::stdin::NonBlockingStdin, Error};
use std::{
    io::{stdout, Read, Write},
    ops::{Deref, DerefMut},
};

use self::sync_stream::NonBlocking;

#[cfg(unix)]
pub(crate) type Proc = crate::process::unix::UnixProcess;
#[cfg(unix)]
pub(crate) type Stream = crate::process::unix::PtyStream;
#[cfg(windows)]
pub(crate) type Proc = crate::process::windows::WinProcess;

#[cfg(not(feature = "async"))]
pub type Session<P = Proc, S = Stream> = sync_session::Session<P, S>;

impl<S> Session<Proc, S>
where
    S: NonBlocking + Write + Read,
{
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
    pub fn interact(&mut self) -> Result<(), Error> {
        let mut stdin = NonBlockingStdin::new()?;
        let stdout = stdout();
        stdin.prepare(self.deref_mut())?;
        let result = crate::interact::InteractOptions::streamed(&mut stdin, stdout)?.interact(self);
        stdin.close(self.deref_mut())?;
        result
    }
}
