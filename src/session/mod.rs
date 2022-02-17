#[cfg(feature = "async")]
mod async_session;
#[cfg(feature = "async")]
mod async_stream;
#[cfg(not(feature = "async"))]
mod sync_session;
// fixme: remove this.
pub mod sync_stream;

#[cfg(feature = "async")]
use crate::process::IntoAsyncStream;

use crate::{process::Process, stream::stdin::Stdin, Error};
use std::io::{stdout, Read, Write};

use self::sync_stream::NonBlocking;

#[cfg(unix)]
pub(crate) type Proc = crate::process::unix::UnixProcess;
#[cfg(all(unix, not(feature = "async")))]
pub(crate) type Stream = crate::process::unix::PtyStream;
#[cfg(all(unix, feature = "async"))]
pub(crate) type Stream = crate::process::unix::AsyncPtyStream;

#[cfg(windows)]
pub(crate) type Proc = crate::process::windows::WinProcess;
#[cfg(windows)]
pub(crate) type Stream = crate::process::windows::ProcessStream;

#[cfg(not(feature = "async"))]
pub type Session<P = Proc, S = Stream> = sync_session::Session<P, S>;

#[cfg(feature = "async")]
pub type Session<P = Proc, S = Stream> = async_session::Session<P, S>;

#[cfg(not(feature = "async"))]
impl Session {
    pub fn spawn(command: <Proc as Process>::Command) -> Result<Self, Error> {
        let mut process = Proc::spawn_command(command)?;
        let stream = process.open_stream()?;
        let session = Self::new(process, stream)?;

        Ok(session)
    }

    pub(crate) fn spawn_cmd(cmd: &str) -> Result<Self, Error> {
        let mut process = Proc::spawn(cmd)?;
        let stream = process.open_stream()?;
        let session = Self::new(process, stream)?;

        Ok(session)
    }
}

#[cfg(not(feature = "async"))]
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
        let mut stdin = Stdin::new(self)?;
        let mut stdout = stdout();
        let result =
            crate::interact::InteractOptions::default().interact(self, &mut stdin, &mut stdout);
        stdin.close(self)?;
        result
    }
}

#[cfg(feature = "async")]
impl Session {
    pub fn spawn(command: <Proc as Process>::Command) -> Result<Self, Error> {
        let mut process = Proc::spawn_command(command)?;
        let stream = process.open_stream()?.into_async_stream()?;
        let session = Self::new(process, stream)?;

        Ok(session)
    }

    pub(crate) fn spawn_cmd(cmd: &str) -> Result<Self, Error> {
        let mut process = Proc::spawn(cmd)?;
        let stream = process.open_stream()?.into_async_stream()?;
        let session = Self::new(process, stream)?;

        Ok(session)
    }
}

#[cfg(feature = "async")]
impl<S> Session<Proc, S>
where
    S: futures_lite::AsyncRead + futures_lite::AsyncWrite + Unpin,
{
    pub async fn interact(&mut self) -> Result<(), Error> {
        let mut stdin = Stdin::new(self)?;
        let mut stdout = stdout();
        let result = crate::interact::InteractOptions::default()
            .interact(self, &mut stdin, &mut stdout)
            .await;
        stdin.close(self)?;
        result
    }
}
