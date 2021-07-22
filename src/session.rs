use crate::{
    error::Error,
    expect::{Match, Needle},
};
use ptyprocess::PtyProcess;
use regex::Regex;
use std::{
    ops::{Deref, DerefMut},
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
