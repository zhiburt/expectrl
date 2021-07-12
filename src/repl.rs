use std::{
    ops::{Deref, DerefMut},
    process::Command,
};

use crate::{error::Error, Session};

/// Spawn a bash session.
///
/// It uses a custom prompt to be able to controll shell better.
#[cfg(feature = "sync")]
pub fn spawn_bash() -> Result<ReplSession, Error> {
    const DEFAULT_PROMPT: &str = "EXPECT_PROMPT>";
    let mut cmd = Command::new("bash");
    cmd.env("PS1", DEFAULT_PROMPT);
    cmd.env_remove("PROMPT_COMMAND");
    let mut bash = ReplSession::new(cmd, DEFAULT_PROMPT, Some("quit"))?;
    
    // read a prompt to make it not available on next read.
    bash.expect_prompt()?;

    Ok(bash)
}

/// Spawn a bash session.
///
/// It uses a custom prompt to be able to controll shell better.
#[cfg(feature = "async")]
pub async fn spawn_bash() -> Result<ReplSession, Error> {
    const DEFAULT_PROMPT: &str = "EXPECT_PROMPT>";
    let mut cmd = Command::new("bash");
    cmd.env("PS1", DEFAULT_PROMPT);
    cmd.env_remove("PROMPT_COMMAND");
    let mut bash = ReplSession::new(cmd, DEFAULT_PROMPT, Some("quit"))?;
    
    // read a prompt to make it not available on next read.
    bash.expect_prompt().await?;

    Ok(bash)
}

/// Spawn default python's IDLE.
pub fn spawn_python() -> Result<ReplSession, Error> {
    let idle = ReplSession::new(Command::new("python"), ">>> ", Some("quit()"))?;
    Ok(idle)
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
    pub fn new<P: AsRef<str>, Q: AsRef<str>>(
        cmd: Command,
        prompt: P,
        quit_command: Option<Q>,
    ) -> Result<Self, Error> {
        let session = Session::spawn_cmd(cmd)?;
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

    /// Get a size in bytes of a prompt, may be usefull for triming it.
    pub fn prompt_len(&self) -> usize {
        self.prompt.as_bytes().len()
    }
}

#[cfg(feature = "async")]
impl ReplSession {
    /// Block until prompt is found
    pub async fn expect_prompt(&mut self) -> Result<(), Error> {
        let prompt = self.prompt.clone();
        self.expect(&prompt).await?;
        Ok(())
    }

    /// Send a command to a repl and verifies that it exited.
    pub async fn execute<S: AsRef<str> + Clone>(&mut self, cmd: S) -> Result<(), Error> {
        self.send_line(cmd.clone()).await?;
        if self.is_echo_on {
            self.expect(cmd.as_ref()).await?;
        }
        self.expect_prompt().await?;
        Ok(())
    }

    /// Sends line to repl (and flush the output).
    ///
    /// If echo_on=true wait for the input to appear.
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
    pub async fn exit(&mut self) -> Result<(), Error> {
        if let Some(quit_command) = self.quit_command.clone() {
            self.session.send_line(quit_command).await?;
        }

        Ok(())
    }
}

#[cfg(feature = "sync")]
impl ReplSession {
    /// Block until prompt is found
    pub fn expect_prompt(&mut self) -> Result<(), Error> {
        let prompt = self.prompt.clone();
        self.expect(&prompt)?;
        Ok(())
    }

    /// Send a command to a repl and verifies that it exited.
    pub fn execute<S: AsRef<str> + Clone>(&mut self, cmd: S) -> Result<(), Error> {
        self.send_line(cmd.clone())?;
        if self.is_echo_on {
            self.expect(cmd.as_ref())?;
        }
        self.expect_prompt()?;
        Ok(())
    }

    /// Sends line to repl (and flush the output).
    ///
    /// If echo_on=true wait for the input to appear.
    pub fn send_line<S: AsRef<str>>(&mut self, line: S) -> Result<(), Error> {
        self.session.send_line(line.as_ref())?;
        if self.is_echo_on {
            self.expect(line.as_ref())?;
        }
        Ok(())
    }
}

#[cfg(feature = "sync")]
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
