use std::{
    ops::{Deref, DerefMut},
    process::Command,
};

use crate::{error::Error, Session};

/// Spawn a bash session.
///
/// It uses a custom prompt to be able to controll shell better.
pub fn spawn_bash() -> Result<ReplSession, Error> {
    // First line of a spawned bash seemingly has an old prompt.
    const DEFAULT_PROMPT: &str = "EXPECT_PROMPT> ";
    let mut cmd = Command::new("bash");
    cmd.env("PS1", DEFAULT_PROMPT);
    let bash = ReplSession::new(cmd, DEFAULT_PROMPT, Some("quit"))?;

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

impl<'a> ReplSession {
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

    /// Block until prompt is found
    pub fn expect_prompt(&mut self) -> Result<(), Error> {
        let prompt = self.prompt.clone();
        self.expect(&prompt)?;
        Ok(())
    }

    /// Send a command to a repl and verifies that it exited.
    pub fn execute(&mut self, cmd: &str) -> Result<(), Error> {
        self.send_line(cmd)?;
        if self.is_echo_on {
            self.expect(cmd)?;
        }
        self.expect_prompt()?;
        Ok(())
    }

    /// Sends line to repl (and flush the output).
    ///
    /// If echo_on=true wait for the input to appear.
    pub fn send_line(&mut self, line: &str) -> Result<(), Error> {
        self.session.send_line(line)?;
        if self.is_echo_on {
            self.expect(line)?;
        }
        Ok(())
    }

    /// Get a size in bytes of a prompt, may be usefull for triming it.
    pub fn prompt_len(&self) -> usize {
        self.prompt.as_bytes().len()
    }
}

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
