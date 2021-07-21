use crate::{
    error::Error,
    expect::{Match, Needle},
};
use ptyprocess::{stream::Stream, PtyProcess, WaitStatus};
use regex::Regex;
use std::{
    ops::{Deref, DerefMut},
    os::unix::prelude::{AsRawFd, FromRawFd},
    process::Command,
    time::{self, Duration},
};

/// Session represents a process and its streams.
/// It controlls process and communication with it.
#[derive(Debug)]
pub struct Session {
    proc: PtyProcess,
    expect_timeout: Option<Duration>,
}

impl Session {
    /// Spawn spawn a cmd process
    pub fn spawn(cmd: &str) -> Result<Self, Error> {
        let args = tokenize_command(cmd);
        if args.is_empty() {
            return Err(Error::CommandParsing);
        }

        let mut command = Command::new(&args[0]);
        command.args(args.iter().skip(1));

        Self::spawn_cmd(command)
    }

    /// Spawn spawns a command
    pub fn spawn_cmd(command: Command) -> Result<Self, Error> {
        let ptyproc = PtyProcess::spawn(command)?;

        Ok(Self {
            proc: ptyproc,
            expect_timeout: Some(Duration::from_millis(10000)),
        })
    }

    /// Expect waits until a pattern is matched.
    ///
    /// It return error if expect_timeout is reached.
    #[cfg(feature = "async")]
    pub async fn expect<E: Needle>(&mut self, expect: E) -> Result<Found, Error> {
        let start = time::Instant::now();
        let mut eof_reached = false;
        let mut buf = Vec::new();
        loop {
            // We read by byte so there's no need for buffering.
            // If it would read by block's we would be required to create an internal buffer
            // and implement std::io::Read and async_io::AsyncRead to use it.
            // But instead we just reuse it from `ptyprocess` via `Deref`.
            //
            // It's worth to use this approch if there's a performance issue.
            match self.proc.try_read_byte().await? {
                Some(None) => eof_reached = true,
                Some(Some(b)) => buf.push(b),
                None => {}
            };

            if let Some(m) = expect.check(&buf, eof_reached)? {
                let buf = buf.drain(..m.end()).collect();
                return Ok(Found::new(buf, m));
            }

            if eof_reached {
                return Err(Error::Eof);
            }

            if let Some(timeout) = self.expect_timeout {
                if start.elapsed() > timeout {
                    return Err(Error::ExpectTimeout);
                }
            }
        }
    }

    /// Expect waits until a pattern is matched.
    ///
    /// It return an error if expect_timeout is reached.
    #[cfg(feature = "sync")]
    pub fn expect<E: Needle>(&mut self, expect: E) -> Result<Found, Error> {
        let start = time::Instant::now();
        let mut eof_reached = false;
        let mut buf = Vec::new();
        loop {
            // We read by byte so there's no need for buffering.
            // If it would read by block's we would be required to create an internal buffer
            // and implement std::io::Read and async_io::AsyncRead to use it.
            // But instead we just reuse it from `ptyprocess` via `Deref`.
            //
            // It's worth to use this approch if there's a performance issue.
            match self.proc.try_read_byte()? {
                Some(None) => eof_reached = true,
                Some(Some(b)) => buf.push(b),
                None => {}
            };

            if let Some(m) = expect.check(&buf, eof_reached)? {
                let buf = buf.drain(..m.end()).collect();
                return Ok(Found::new(buf, m));
            }

            if eof_reached {
                return Err(Error::Eof);
            }

            if let Some(timeout) = self.expect_timeout {
                if start.elapsed() > timeout {
                    return Err(Error::ExpectTimeout);
                }
            }
        }
    }

    /// Set the pty session's expect timeout.
    pub fn set_expect_timeout(&mut self, expect_timeout: Option<Duration>) {
        self.expect_timeout = expect_timeout;
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
    #[cfg(feature = "sync")]
    pub fn interact(&mut self) -> Result<WaitStatus, Error> {
        use std::io::Write;

        use ptyprocess::ControlCode;

        // flush buffers
        self.flush()?;

        let stdin = unsafe { std::fs::File::from_raw_fd(std::io::stdin().as_raw_fd()) };
        let mut stdin_stream = Stream::new(stdin);
        let mut buf = [0; 512];
        loop {
            let status = self.status();
            if !matches!(status, Ok(WaitStatus::StillAlive)) {
                return Ok(status?);
            }

            if let Some(n) = self.try_read(&mut buf)? {
                std::io::stdout().write_all(&buf[..n])?;
                std::io::stdout().flush()?;
            }

            if let Some(n) = stdin_stream.try_read(&mut buf)? {
                for i in 0..n {
                    // Ctrl-]
                    if buf[i] == ControlCode::GroupSeparator.into() {
                        // it might be too much to call a `status()` here,
                        // do it just in case.
                        return Ok(self.status()?);
                    }

                    self.write_all(&buf[i..i + 1])?;
                }
            }
        }
    }

    #[cfg(feature = "async")]
    pub async fn interact(&mut self) -> Result<WaitStatus, Error> {
        use futures_lite::AsyncWriteExt;
        use ptyprocess::ControlCode;
        use std::io::Write;

        // flush buffers
        self.flush().await?;

        let stdin = unsafe { std::fs::File::from_raw_fd(std::io::stdin().as_raw_fd()) };
        let mut stdin_stream = Stream::new(stdin);
        let mut buf = [0; 512];
        loop {
            let status = self.status();
            if !matches!(status, Ok(WaitStatus::StillAlive)) {
                return Ok(status?);
            }

            if let Some(n) = self.try_read(&mut buf).await? {
                std::io::stdout().write_all(&buf[..n])?;
                std::io::stdout().flush()?;
            }

            if let Some(n) = stdin_stream.try_read(&mut buf).await? {
                for i in 0..n {
                    // Ctrl-]
                    if buf[i] == ControlCode::GroupSeparator.into() {
                        // it might be too much to call a `status()` here,
                        // do it just in case.
                        return Ok(self.status()?);
                    }

                    self.write_all(&buf[i..i + 1]).await?;
                }
            }
        }
    }
}

impl Deref for Session {
    type Target = PtyProcess;

    fn deref(&self) -> &Self::Target {
        &self.proc
    }
}

impl DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.proc
    }
}

/// Found is a represention of a matched pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    buf: Vec<u8>,
    m: Match,
}

impl Found {
    /// New returns an instance of Found.
    pub fn new(buf: Vec<u8>, m: Match) -> Self {
        Self { buf, m }
    }

    /// Found_match returns a matched bytes.
    pub fn found_match(&self) -> &[u8] {
        &self.buf[self.m.start()..self.m.end()]
    }

    /// Before_match returns a bytes before match.
    pub fn before_match(&self) -> &[u8] {
        &self.buf[..self.m.start()]
    }
}

/// Turn e.g. "prog arg1 arg2" into ["prog", "arg1", "arg2"]
/// It takes care of single and double quotes but,
///
/// It doesn't cover all edge cases.
/// So it may not be compatible with real shell arguments parsing.
fn tokenize_command(program: &str) -> Vec<String> {
    let re = Regex::new(r#""[^"]+"|'[^']+'|[^'" ]+"#).unwrap();
    let mut res = vec![];
    for cap in re.captures_iter(program) {
        res.push(cap[0].to_string());
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_command() {
        let res = tokenize_command("prog arg1 arg2");
        assert_eq!(vec!["prog", "arg1", "arg2"], res);

        let res = tokenize_command("prog -k=v");
        assert_eq!(vec!["prog", "-k=v"], res);

        let res = tokenize_command("prog 'my text'");
        assert_eq!(vec!["prog", "'my text'"], res);

        let res = tokenize_command(r#"prog "my text""#);
        assert_eq!(vec!["prog", r#""my text""#], res);
    }

    #[test]
    fn test_spawn_no_command() {
        assert!(matches!(
            Session::spawn("").unwrap_err(),
            Error::CommandParsing
        ));
    }
}
