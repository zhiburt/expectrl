//! This module contains a list of special Sessions that can be spawned.

use std::{
    io::{self, BufRead, Read, Write},
    pin::Pin,
    task::{Context, Poll},
};

#[cfg(unix)]
use std::process::Command;

use crate::{
    error::Error,
    process::{Healthcheck, Termios},
    session::OsSession,
    AsyncExpect, Captures, Expect, Needle,
};

use crate::spawn;

#[cfg(not(feature = "async"))]
use crate::process::NonBlocking;

use futures_lite::io::AsyncBufRead;
#[cfg(feature = "async")]
use futures_lite::{AsyncRead, AsyncWrite};

type ExpectResult<T> = Result<T, Error>;

/// Spawn a bash session.
///
/// It uses a custom prompt to be able to controll shell better.
///
/// If you wan't to use [Session::interact] method it is better to use just Session.
/// Because we don't handle echoes here (currently). Ideally we need to.
#[cfg(unix)]
#[cfg(not(feature = "async"))]
pub fn spawn_bash() -> ExpectResult<ReplSession<Session<OsProcess, OsProcessStream>>> {
    const DEFAULT_PROMPT: &str = "EXPECT_PROMPT";

    let mut cmd = Command::new("bash");
    let _ = cmd.env("PS1", DEFAULT_PROMPT);
    // bind 'set enable-bracketed-paste off' turns off paste mode,
    // without it each command in bash starts and ends with an invisible sequence.
    //
    // We might need to turn it off optionally?
    let _ = cmd.env(
        "PROMPT_COMMAND",
        "PS1=EXPECT_PROMPT; unset PROMPT_COMMAND; bind 'set enable-bracketed-paste off'",
    );

    let session = crate::session::Session::spawn(cmd)?;

    let mut bash = ReplSession::new(session, DEFAULT_PROMPT);
    bash.set_quit_command("quit");

    // read a prompt to make it not available on next read.
    //
    // fix: somehow this line causes a different behaviour in iteract method.
    //      the issue most likely that with this line in interact mode ENTER produces CTRL-M
    //      when without the line it produces \r\n

    bash.expect_prompt()?;

    Ok(bash)
}

/// Spawn a bash session.
///
/// It uses a custom prompt to be able to controll shell better.
#[cfg(unix)]
#[cfg(feature = "async")]
pub async fn spawn_bash() -> ExpectResult<ReplSession<OsSession>> {
    const DEFAULT_PROMPT: &str = "EXPECT_PROMPT";
    let mut cmd = Command::new("bash");
    let _ = cmd.env("PS1", DEFAULT_PROMPT);
    // bind 'set enable-bracketed-paste off' turns off paste mode,
    // without it each command in bash starts and ends with an invisible sequence.
    //
    // We might need to turn it off optionally?
    let _ = cmd.env(
        "PROMPT_COMMAND",
        "PS1=EXPECT_PROMPT; unset PROMPT_COMMAND; bind 'set enable-bracketed-paste off'",
    );

    let session = crate::session::Session::spawn(cmd)?;

    let mut bash = ReplSession::new(session, DEFAULT_PROMPT);
    bash.set_quit_command("quit");
    bash.set_echo(false);

    // read a prompt to make it not available on next read.
    bash.expect_prompt().await?;

    Ok(bash)
}

/// Spawn default python's IDLE.
#[cfg(not(feature = "async"))]
pub fn spawn_python() -> ExpectResult<ReplSession<Session<OsProcess, OsProcessStream>>> {
    // todo: check windows here
    // If we spawn it as ProcAttr::default().commandline("python") it will spawn processes endlessly....

    let session = spawn("python")?;

    let mut idle = ReplSession::new(session, ">>> ");
    idle.set_quit_command("quit()");
    idle.expect_prompt()?;

    Ok(idle)
}

/// Spawn default python's IDLE.
#[cfg(feature = "async")]
pub async fn spawn_python() -> ExpectResult<ReplSession<OsSession>> {
    // todo: check windows here
    // If we spawn it as ProcAttr::default().commandline("python") it will spawn processes endlessly....

    let session = spawn("python")?;

    let mut idle = ReplSession::new(session, ">>> ");
    idle.set_quit_command("quit()");
    idle.set_echo(false);

    idle.expect_prompt().await?;
    Ok(idle)
}

/// Spawn a powershell session.
///
/// It uses a custom prompt to be able to controll the shell.
#[cfg(windows)]
#[cfg(not(feature = "async"))]
pub fn spawn_powershell() -> ExpectResult<ReplSession> {
    const DEFAULT_PROMPT: &str = "EXPECTED_PROMPT>";
    let session = spawn("pwsh -NoProfile -NonInteractive -NoLogo")?;
    let mut powershell = ReplSession::new(
        session,
        DEFAULT_PROMPT.to_owned(),
        Some("exit".to_owned()),
        true,
    );

    // https://stackoverflow.com/questions/5725888/windows-powershell-changing-the-command-prompt
    let _ = powershell.execute(format!(
        r#"function prompt {{ "{}"; return " " }}"#,
        DEFAULT_PROMPT
    ))?;

    // https://stackoverflow.com/questions/69063656/is-it-possible-to-stop-powershell-wrapping-output-in-ansi-sequences/69063912#69063912
    // https://docs.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_ansi_terminals?view=powershell-7.2#disabling-ansi-output
    let _ =
        powershell.execute(r#"[System.Environment]::SetEnvironmentVariable("TERM", "dumb")"#)?;
    let _ = powershell
        .execute(r#"[System.Environment]::SetEnvironmentVariable("TERM", "NO_COLOR")"#)?;

    Ok(powershell)
}

/// Spawn a powershell session.
///
/// It uses a custom prompt to be able to controll the shell.
#[cfg(windows)]
#[cfg(feature = "async")]
pub async fn spawn_powershell() -> ExpectResult<ReplSession> {
    const DEFAULT_PROMPT: &str = "EXPECTED_PROMPT>";
    let session = spawn("pwsh -NoProfile -NonInteractive -NoLogo")?;
    let mut powershell = ReplSession::new(
        session,
        DEFAULT_PROMPT.to_owned(),
        Some("exit".to_owned()),
        true,
    );

    // https://stackoverflow.com/questions/5725888/windows-powershell-changing-the-command-prompt
    let _ = powershell
        .execute(format!(
            r#"function prompt {{ "{}"; return " " }}"#,
            DEFAULT_PROMPT
        ))
        .await?;

    // https://stackoverflow.com/questions/69063656/is-it-possible-to-stop-powershell-wrapping-output-in-ansi-sequences/69063912#69063912
    // https://docs.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_ansi_terminals?view=powershell-7.2#disabling-ansi-output
    let _ = powershell
        .execute(r#"[System.Environment]::SetEnvironmentVariable("TERM", "dumb")"#)
        .await?;
    let _ = powershell
        .execute(r#"[System.Environment]::SetEnvironmentVariable("TERM", "NO_COLOR")"#)
        .await?;

    Ok(powershell)
}

/// A repl session: e.g. bash or the python shell:
/// you have a prompt where a user inputs commands and the shell
/// which executes them and manages IO streams.
#[derive(Debug)]
pub struct ReplSession<S> {
    /// A pseudo-teletype session with a spawned process.
    session: S,
    /// The prompt, used for `wait_for_prompt`,
    /// e.g. ">>> " for python.
    prompt: String,
    /// A command which will be called before termination.
    quit_command: Option<String>,
    /// Flag to see if a echo is turned on.
    is_echo_on: bool,
}

impl<S> ReplSession<S> {
    /// Spawn function creates a repl session.
    ///
    /// The argument list is:
    ///     - session; a spawned session which repl will wrap.
    ///     - prompt; a string which will identify that the command was run.
    ///     - quit_command; a command which will be called when [ReplSession] instance is dropped.
    ///     - is_echo_on; determines whether the prompt check will be done twice.
    pub fn new(session: S, prompt: impl Into<String>) -> Self {
        Self {
            session,
            prompt: prompt.into(),
            quit_command: None,
            is_echo_on: false,
        }
    }

    /// Set echo settings to be expected.
    pub fn set_echo(&mut self, on: bool) {
        self.is_echo_on = on;
    }

    /// Set quit command which will be called on `exit`.
    pub fn set_quit_command(&mut self, cmd: impl Into<String>) {
        self.quit_command = Some(cmd.into());
    }

    /// Get a used prompt.
    pub fn get_prompt(&self) -> &str {
        &self.prompt
    }

    /// Get a used quit command.
    pub fn get_quit_command(&self) -> Option<&str> {
        self.quit_command.as_deref()
    }

    /// Get a echo settings.
    pub fn is_echo(&self) -> bool {
        self.is_echo_on
    }

    /// Get an inner session.
    pub fn into_session(self) -> S {
        self.session
    }

    /// Get an inner session.
    pub fn get_session(&self) -> &S {
        &self.session
    }

    /// Get an inner session.
    pub fn get_session_mut(&mut self) -> &mut S {
        &mut self.session
    }
}

#[cfg(not(feature = "async"))]
impl<S> ReplSession<S>
where
    S: Expect,
{
    /// Block until prompt is found
    pub fn expect_prompt(&mut self) -> Result<(), Error> {
        let _ = self._expect_prompt()?;
        Ok(())
    }

    fn _expect_prompt(&mut self) -> Result<Captures, Error> {
        self.session.expect(&self.prompt)
    }
}

#[cfg(feature = "async")]
impl<S> ReplSession<S>
where
    S: AsyncExpect + Unpin,
{
    /// Block until prompt is found
    pub async fn expect_prompt(&mut self) -> Result<(), Error> {
        let _ = self._expect_prompt().await?;
        Ok(())
    }

    async fn _expect_prompt(&mut self) -> Result<Captures, Error> {
        self.session.expect(&self.prompt).await
    }
}

#[cfg(not(feature = "async"))]
impl<S> ReplSession<S>
where
    S: Expect,
{
    /// Send a command to a repl and verifies that it exited.
    /// Returning it's output.
    pub fn execute<C>(&mut self, cmd: C) -> Result<Vec<u8>, Error>
    where
        C: AsRef<str>,
    {
        self.send_line(cmd)?;
        let found = self._expect_prompt()?;
        let out = found.before().to_vec();

        Ok(out)
    }

    /// Sends line to repl (and flush the output).
    ///
    /// If echo_on=true wait for the input to appear.
    #[cfg(not(feature = "async"))]
    pub fn send_line<L>(&mut self, line: L) -> Result<(), Error>
    where
        L: AsRef<str>,
    {
        let text = line.as_ref();
        self.session.send_line(text)?;
        if self.is_echo_on {
            let _ = self.get_session_mut().expect(line.as_ref())?;
        }

        Ok(())
    }

    /// Send a quit command.
    ///
    /// In async version we it won't be send on Drop so,
    /// If you wan't it to be send you must do it yourself.
    pub fn exit(&mut self) -> Result<(), Error> {
        if let Some(quit_command) = &self.quit_command {
            self.session.send_line(quit_command)?;
        }

        Ok(())
    }
}

#[cfg(feature = "async")]
impl<S> ReplSession<S>
where
    S: AsyncExpect + Unpin,
{
    /// Send a command to a repl and verifies that it exited.
    pub async fn execute(&mut self, cmd: impl AsRef<str>) -> Result<Vec<u8>, Error> {
        self.send_line(cmd).await?;
        let found = self._expect_prompt().await?;
        Ok(found.before().to_vec())
    }

    /// Sends line to repl (and flush the output).
    ///
    /// If echo_on=true wait for the input to appear.
    pub async fn send_line(&mut self, line: impl AsRef<str>) -> Result<(), Error> {
        self.session.send_line(line.as_ref()).await?;
        if self.is_echo_on {
            let _ = self.expect(line.as_ref()).await?;
        }
        Ok(())
    }

    /// Send a quit command.
    ///
    /// In async version we it won't be send on Drop so,
    /// If you wan't it to be send you must do it yourself.
    pub async fn exit(&mut self) -> Result<(), Error> {
        if let Some(quit_command) = &self.quit_command {
            self.session.send_line(quit_command).await?;
        }

        Ok(())
    }
}

impl<S> Healthcheck for ReplSession<S>
where
    S: Healthcheck,
{
    type Status = S::Status;

    fn get_status(&self) -> io::Result<Self::Status> {
        self.get_session().get_status()
    }

    fn is_alive(&self) -> io::Result<bool> {
        self.get_session().is_alive()
    }
}

impl<S> Termios for ReplSession<S>
where
    S: Termios,
{
    fn is_echo(&self) -> io::Result<bool> {
        self.get_session().is_echo()
    }

    fn set_echo(&mut self, on: bool) -> io::Result<bool> {
        self.get_session_mut().set_echo(on)
    }
}

impl<S> Expect for ReplSession<S>
where
    S: Expect,
{
    fn expect<N>(&mut self, needle: N) -> ExpectResult<Captures>
    where
        N: Needle,
    {
        S::expect(self.get_session_mut(), needle)
    }

    fn check<N>(&mut self, needle: N) -> ExpectResult<Captures>
    where
        N: Needle,
    {
        S::check(self.get_session_mut(), needle)
    }

    fn is_matched<N>(&mut self, needle: N) -> ExpectResult<bool>
    where
        N: Needle,
    {
        S::is_matched(self.get_session_mut(), needle)
    }

    fn send<B>(&mut self, buf: B) -> ExpectResult<()>
    where
        B: AsRef<[u8]>,
    {
        S::send(self.get_session_mut(), buf)
    }

    fn send_line<B>(&mut self, buf: B) -> ExpectResult<()>
    where
        B: AsRef<[u8]>,
    {
        S::send_line(self.get_session_mut(), buf)
    }
}

#[cfg(feature = "async")]
impl<S> AsyncExpect for ReplSession<S>
where
    S: AsyncExpect,
{
    async fn expect<N>(&mut self, needle: N) -> ExpectResult<Captures>
    where
        N: Needle,
    {
        S::expect(self.get_session_mut(), needle).await
    }

    async fn check<N>(&mut self, needle: N) -> ExpectResult<Captures>
    where
        N: Needle,
    {
        S::check(self.get_session_mut(), needle).await
    }

    async fn is_matched<N>(&mut self, needle: N) -> ExpectResult<bool>
    where
        N: Needle,
    {
        S::is_matched(self.get_session_mut(), needle).await
    }

    async fn send<B>(&mut self, buf: B) -> ExpectResult<()>
    where
        B: AsRef<[u8]>,
    {
        S::send(self.get_session_mut(), buf).await
    }

    async fn send_line<B>(&mut self, buf: B) -> ExpectResult<()>
    where
        B: AsRef<[u8]>,
    {
        S::send_line(self.get_session_mut(), buf).await
    }
}

impl<S> Write for ReplSession<S>
where
    S: Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        S::write(self.get_session_mut(), buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        S::flush(self.get_session_mut())
    }
}

impl<S> Read for ReplSession<S>
where
    S: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        S::read(self.get_session_mut(), buf)
    }
}

impl<S> BufRead for ReplSession<S>
where
    S: BufRead,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        S::fill_buf(self.get_session_mut())
    }

    fn consume(&mut self, amt: usize) {
        S::consume(self.get_session_mut(), amt)
    }
}

impl<S> AsyncWrite for ReplSession<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        S::poll_write(Pin::new(self.get_session_mut()), cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        S::poll_flush(Pin::new(self.get_session_mut()), cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        S::poll_close(Pin::new(self.get_session_mut()), cx)
    }
}

impl<S> AsyncRead for ReplSession<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        S::poll_read(Pin::new(self.get_session_mut()), cx, buf)
    }
}

impl<S> AsyncBufRead for ReplSession<S>
where
    S: AsyncBufRead + Unpin,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        S::poll_fill_buf(Pin::new(self.get_mut().get_session_mut()), cx)
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        S::consume(Pin::new(self.get_session_mut()), amt)
    }
}
