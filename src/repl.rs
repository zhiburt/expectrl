//! This module contains a list of special Sessions that can be spawned.

use crate::{error::Error, session::Found, Session};
use std::ops::{Deref, DerefMut};

#[cfg(unix)]
use std::process::Command;

#[cfg(windows)]
use conpty::ProcAttr;

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
    cmd.env("PS1", DEFAULT_PROMPT);
    // bind 'set enable-bracketed-paste off' turns off paste mode,
    // without it each command in bash starts and ends with an invisible sequence.
    //
    // We might need to turn it off optionally?
    cmd.env(
        "PROMPT_COMMAND",
        "PS1=EXPECT_PROMPT; unset PROMPT_COMMAND; bind 'set enable-bracketed-paste off'",
    );
    let mut bash = ReplSession::spawn(cmd, DEFAULT_PROMPT, Some("quit"))?;

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
    let mut bash = ReplSession::spawn(cmd, DEFAULT_PROMPT, Some("quit"))?;

    // read a prompt to make it not available on next read.
    bash.expect_prompt().await?;

    Ok(bash)
}

/// Spawn default python's IDLE.
#[cfg(not(feature = "async"))]
pub fn spawn_python() -> Result<ReplSession, Error> {
    #[cfg(unix)]
    {
        let mut idle = ReplSession::spawn(Command::new("python"), ">>> ", Some("quit()"))?;
        idle._expect_prompt()?;
        Ok(idle)
    }
    #[cfg(windows)]
    {
        // If we spawn it as ProcAttr::default().commandline("python") it will spawn processes endlessly....
        let mut idle =
            ReplSession::spawn(ProcAttr::cmd("python".to_string()), ">>> ", Some("quit()"))?;
        idle._expect_prompt()?;
        Ok(idle)
    }
}

/// Spawn default python's IDLE.
#[cfg(feature = "async")]
#[cfg(unix)]
pub async fn spawn_python() -> Result<ReplSession, Error> {
    let mut idle = ReplSession::spawn(Command::new("python"), ">>> ", Some("quit()"))?;
    idle._expect_prompt().await?;
    Ok(idle)
}

/// Spawn a powershell session.
///
/// It uses a custom prompt to be able to controll the shell.
#[cfg(windows)]
pub fn spawn_powershell() -> Result<ReplSession, Error> {
    const DEFAULT_PROMPT: &str = "EXPECTED_PROMPT>";
    let mut powershell = ReplSession::spawn(
        ProcAttr::default().commandline(r"pwsh -NoProfile -NonInteractive -NoLogo".to_string()),
        DEFAULT_PROMPT,
        Some("exit"),
    )?;
    powershell.is_echo_on = true;

    // https://stackoverflow.com/questions/5725888/windows-powershell-changing-the-command-prompt
    powershell.execute(format!(
        r#"function prompt {{ "{}"; return " " }}"#,
        DEFAULT_PROMPT
    ))?;

    // https://stackoverflow.com/questions/69063656/is-it-possible-to-stop-powershell-wrapping-output-in-ansi-sequences/69063912#69063912
    // https://docs.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_ansi_terminals?view=powershell-7.2#disabling-ansi-output
    powershell.execute(r#"[System.Environment]::SetEnvironmentVariable("TERM", "dumb")"#)?;
    powershell.execute(r#"[System.Environment]::SetEnvironmentVariable("TERM", "NO_COLOR")"#)?;

    Ok(powershell)
}

/// A repl session: e.g. bash or the python shell:
/// you have a prompt where a user inputs commands and the shell
/// which executes them and manages IO streams.
pub struct ReplSession {
    /// The prompt, used for `wait_for_prompt`,
    /// e.g. ">>> " for python.
    prompt: String,
    /// A pseudo-teletype session with a spawned process.
    session: Session,
    /// A command which will be called before termination.
    quit_command: Option<String>,
    /// Flag to see if a echo is turned on.
    is_echo_on: bool,
}

impl ReplSession {
    #[cfg(unix)]
    pub fn spawn<P: AsRef<str>, Q: AsRef<str>>(
        cmd: Command,
        prompt: P,
        quit_command: Option<Q>,
    ) -> Result<Self, Error> {
        let session = Session::spawn(cmd)?;
        let is_echo_on = session.get_echo()?;
        let prompt = prompt.as_ref().to_owned();
        let quit_command = quit_command.map(|q| q.as_ref().to_owned());

        Ok(Self {
            prompt,
            session,
            quit_command,
            is_echo_on,
        })
    }

    #[cfg(windows)]
    pub fn spawn<P: AsRef<str>, Q: AsRef<str>>(
        attr: crate::ProcAttr,
        prompt: P,
        quit_command: Option<Q>,
    ) -> Result<Self, Error> {
        let session = Session::spawn(attr)?;
        let prompt = prompt.as_ref().to_owned();
        let quit_command = quit_command.map(|q| q.as_ref().to_owned());

        Ok(Self {
            prompt,
            session,
            quit_command,
            is_echo_on: false,
        })
    }

    /// Get a size in bytes of a prompt, may be usefull for triming it.
    pub fn prompt_len(&self) -> usize {
        self.prompt.as_bytes().len()
    }
}

impl ReplSession {
    /// Block until prompt is found
    #[cfg(not(feature = "async"))]
    pub fn expect_prompt(&mut self) -> Result<(), Error> {
        self._expect_prompt()?;
        Ok(())
    }

    /// Block until prompt is found
    #[cfg(feature = "async")]
    pub async fn expect_prompt(&mut self) -> Result<(), Error> {
        self._expect_prompt().await?;
        Ok(())
    }

    #[cfg(not(feature = "async"))]
    fn _expect_prompt(&mut self) -> Result<Found, Error> {
        let prompt = self.prompt.clone();
        self.expect(prompt)
    }

    #[cfg(feature = "async")]
    async fn _expect_prompt(&mut self) -> Result<Found, Error> {
        let prompt = self.prompt.clone();
        self.expect(prompt).await
    }

    /// Send a command to a repl and verifies that it exited.
    /// Returning it's output.
    #[cfg(not(feature = "async"))]
    pub fn execute<S: AsRef<str> + Clone>(&mut self, cmd: S) -> Result<Vec<u8>, Error> {
        self.send_line(cmd)?;
        let found = self._expect_prompt()?;
        Ok(found.before().to_vec())
    }

    /// Send a command to a repl and verifies that it exited.
    #[cfg(feature = "async")]
    pub async fn execute<S: AsRef<str> + Clone>(&mut self, cmd: S) -> Result<Vec<u8>, Error> {
        self.send_line(cmd).await?;
        let found = self._expect_prompt().await?;
        Ok(found.before().to_vec())
    }

    /// Sends line to repl (and flush the output).
    ///
    /// If echo_on=true wait for the input to appear.
    #[cfg(not(feature = "async"))]
    pub fn send_line<S: AsRef<str>>(&mut self, line: S) -> Result<(), Error> {
        self.session.send_line(line.as_ref())?;
        if self.is_echo_on {
            self.expect(line.as_ref())?;
        }
        Ok(())
    }

    /// Sends line to repl (and flush the output).
    ///
    /// If echo_on=true wait for the input to appear.
    #[cfg(feature = "async")]
    pub async fn send_line<S: AsRef<str>>(&mut self, line: S) -> Result<(), Error> {
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
    #[cfg(feature = "async")]
    pub async fn exit(&mut self) -> Result<(), Error> {
        if let Some(quit_command) = self.quit_command.clone() {
            self.session.send_line(quit_command).await?;
        }

        Ok(())
    }
}

#[cfg(not(feature = "async"))]
impl Drop for ReplSession {
    fn drop(&mut self) {
        if let Some(quit_command) = self.quit_command.clone() {
            self.send_line(&quit_command).unwrap()
        }
    }
}

impl Deref for ReplSession {
    type Target = Session;

    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl DerefMut for ReplSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session
    }
}
