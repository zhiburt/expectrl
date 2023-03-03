//! This module contains a list of special Sessions that can be spawned.

use crate::{
    error::Error,
    process::NonBlocking,
    session::{Proc, Stream},
    Captures, Session,
};
use std::{
    io::{Read, Write},
    ops::{Deref, DerefMut},
};

#[cfg(unix)]
use std::process::Command;

use crate::spawn;

#[cfg(feature = "async")]
use futures_lite::{AsyncRead, AsyncWrite};

/// Spawn a bash session.
///
/// It uses a custom prompt to be able to controll shell better.
///
/// If you wan't to use [Session::interact] method it is better to use just Session.
/// Because we don't handle echoes here (currently). Ideally we need to.
#[cfg(unix)]
#[cfg(not(feature = "async"))]
pub fn spawn_bash() -> Result<ReplSession, Error> {
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

    let mut bash = ReplSession::new(
        session,
        DEFAULT_PROMPT.to_string(),
        Some("quit".to_string()),
        false,
    );

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
pub async fn spawn_bash() -> Result<ReplSession, Error> {
    const DEFAULT_PROMPT: &str = "EXPECT_PROMPT";
    let mut cmd = Command::new("bash");
    cmd.env("PS1", DEFAULT_PROMPT);
    // bind 'set enable-bracketed-paste off' turns off paste mode,
    // without it each command in bash starts and ends with an invisible sequence.
    //
    // We might need to turn it off optionally?
    cmd.env(
        "PROMPT_COMMAND",
        "PS1=EXPECT_PROMPT; unset PROMPT_COMMAND; bind 'set enable-bracketed-paste off'",
    );

    let session = crate::session::Session::spawn(cmd)?;

    let mut bash = ReplSession::new(
        session,
        DEFAULT_PROMPT.to_string(),
        Some("quit".to_string()),
        false,
    );

    // read a prompt to make it not available on next read.
    bash.expect_prompt().await?;

    Ok(bash)
}

/// Spawn default python's IDLE.
#[cfg(not(feature = "async"))]
pub fn spawn_python() -> Result<ReplSession, Error> {
    // todo: check windows here
    // If we spawn it as ProcAttr::default().commandline("python") it will spawn processes endlessly....

    let session = spawn("python")?;

    let mut idle = ReplSession::new(session, ">>> ".to_owned(), Some("quit()".to_owned()), false);
    idle.expect_prompt()?;
    Ok(idle)
}

/// Spawn default python's IDLE.
#[cfg(feature = "async")]
pub async fn spawn_python() -> Result<ReplSession, Error> {
    // todo: check windows here
    // If we spawn it as ProcAttr::default().commandline("python") it will spawn processes endlessly....

    let session = spawn("python")?;

    let mut idle = ReplSession::new(session, ">>> ".to_owned(), Some("quit()".to_owned()), false);
    idle.expect_prompt().await?;
    Ok(idle)
}

/// Spawn a powershell session.
///
/// It uses a custom prompt to be able to controll the shell.
#[cfg(windows)]
#[cfg(not(feature = "async"))]
pub fn spawn_powershell() -> Result<ReplSession, Error> {
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
pub async fn spawn_powershell() -> Result<ReplSession, Error> {
    const DEFAULT_PROMPT: &str = "EXPECTED_PROMPT>";
    let session = spawn("pwsh -NoProfile -NonInteractive -NoLogo")?;
    let mut powershell = ReplSession::new(
        session,
        DEFAULT_PROMPT.to_owned(),
        Some("exit".to_owned()),
        true,
    );

    // https://stackoverflow.com/questions/5725888/windows-powershell-changing-the-command-prompt
    powershell
        .execute(format!(
            r#"function prompt {{ "{}"; return " " }}"#,
            DEFAULT_PROMPT
        ))
        .await?;

    // https://stackoverflow.com/questions/69063656/is-it-possible-to-stop-powershell-wrapping-output-in-ansi-sequences/69063912#69063912
    // https://docs.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_ansi_terminals?view=powershell-7.2#disabling-ansi-output
    powershell
        .execute(r#"[System.Environment]::SetEnvironmentVariable("TERM", "dumb")"#)
        .await?;
    powershell
        .execute(r#"[System.Environment]::SetEnvironmentVariable("TERM", "NO_COLOR")"#)
        .await?;

    Ok(powershell)
}

/// A repl session: e.g. bash or the python shell:
/// you have a prompt where a user inputs commands and the shell
/// which executes them and manages IO streams.
#[derive(Debug)]
pub struct ReplSession<P = Proc, S = Stream> {
    /// The prompt, used for `wait_for_prompt`,
    /// e.g. ">>> " for python.
    prompt: String,
    /// A pseudo-teletype session with a spawned process.
    session: Session<P, S>,
    /// A command which will be called before termination.
    quit_command: Option<String>,
    /// Flag to see if a echo is turned on.
    is_echo_on: bool,
}

impl<P, S> ReplSession<P, S> {
    /// Spawn function creates a repl session.
    ///
    /// The argument list is:
    ///     - session; a spawned session which repl will wrap.
    ///     - prompt; a string which will identify that the command was run.
    ///     - quit_command; a command which will be called when [ReplSession] instance is dropped.
    ///     - is_echo_on; determines whether the prompt check will be done twice.
    pub fn new(
        session: Session<P, S>,
        prompt: String,
        quit_command: Option<String>,
        is_echo_on: bool,
    ) -> Self {
        Self {
            session,
            prompt,
            quit_command,
            is_echo_on,
        }
    }

    /// Update an underlying session.
    ///
    /// Can be used to set a logger for example.
    pub fn upgrade_session<NP, NS, F>(self, build_session: F) -> Result<ReplSession<NP, NS>, Error>
    where
        F: FnOnce(Session<P, S>) -> Result<Session<NP, NS>, Error>,
    {
        let session = build_session(self.session)?;
        Ok(ReplSession::new(
            session,
            self.prompt,
            self.quit_command,
            self.is_echo_on,
        ))
    }
}

#[cfg(not(feature = "async"))]
impl<P, S: Read + NonBlocking> ReplSession<P, S> {
    /// Block until prompt is found
    pub fn expect_prompt(&mut self) -> Result<(), Error> {
        let _ = self._expect_prompt()?;
        Ok(())
    }

    fn _expect_prompt(&mut self) -> Result<Captures, Error> {
        let prompt = self.prompt.clone();
        self.expect(prompt)
    }
}

#[cfg(feature = "async")]
impl<P, S: AsyncRead + Unpin> ReplSession<P, S> {
    /// Block until prompt is found
    pub async fn expect_prompt(&mut self) -> Result<(), Error> {
        let _ = self._expect_prompt().await?;
        Ok(())
    }

    async fn _expect_prompt(&mut self) -> Result<Captures, Error> {
        let prompt = self.prompt.clone();
        self.expect(prompt).await
    }
}

#[cfg(not(feature = "async"))]
impl<P, S: Read + NonBlocking + Write> ReplSession<P, S> {
    /// Send a command to a repl and verifies that it exited.
    /// Returning it's output.
    pub fn execute<SS: AsRef<str> + Clone>(&mut self, cmd: SS) -> Result<Vec<u8>, Error> {
        self.send_line(cmd)?;
        let found = self._expect_prompt()?;
        Ok(found.before().to_vec())
    }

    /// Sends line to repl (and flush the output).
    ///
    /// If echo_on=true wait for the input to appear.
    #[cfg(not(feature = "async"))]
    pub fn send_line<SS: AsRef<str>>(&mut self, line: SS) -> Result<(), Error> {
        self.session.send_line(line.as_ref())?;
        if self.is_echo_on {
            let _ = self.expect(line.as_ref())?;
        }
        Ok(())
    }

    /// Send a quit command.
    ///
    /// In async version we it won't be send on Drop so,
    /// If you wan't it to be send you must do it yourself.
    pub fn exit(&mut self) -> Result<(), Error> {
        if let Some(quit_command) = self.quit_command.clone() {
            self.session.send_line(quit_command)?;
        }

        Ok(())
    }
}

#[cfg(feature = "async")]
impl<P, S: AsyncRead + AsyncWrite + Unpin> ReplSession<P, S> {
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
            self.expect(line.as_ref()).await?;
        }
        Ok(())
    }

    /// Send a quit command.
    ///
    /// In async version we it won't be send on Drop so,
    /// If you wan't it to be send you must do it yourself.
    pub async fn exit(&mut self) -> Result<(), Error> {
        if let Some(quit_command) = self.quit_command.clone() {
            self.session.send_line(quit_command).await?;
        }

        Ok(())
    }
}

impl<P, S> Deref for ReplSession<P, S> {
    type Target = Session<P, S>;

    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl<P, S> DerefMut for ReplSession<P, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session
    }
}
