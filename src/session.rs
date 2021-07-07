/*
    - test why tests with new lines fails
    - expect set of calls
    - proc methods: kill wait etc.
*/

use crate::{
    error::Error,
    expect::{Expect, Match},
};
use ptyprocess::PtyProcess;
use std::{
    ops::{Deref, DerefMut},
    process::Command,
    time::{self, Duration},
};

pub struct Session {
    proc: PtyProcess,
    expect_timeout: Option<Duration>,
}

impl Session {
    pub fn spawn(cmd: &str) -> Result<Self, Error> {
        let command = build_command(cmd)?;
        let ptyproc = PtyProcess::spawn(command)?;

        Ok(Self {
            proc: ptyproc,
            expect_timeout: Some(Duration::from_millis(10000)),
        })
    }

    pub fn expect<E: Expect>(&mut self, expect: E) -> Result<Found, Error> {
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

            if let Some(m) = expect.expect(&buf, eof_reached)? {
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

pub struct Found {
    buf: Vec<u8>,
    m: Match,
}

impl Found {
    pub fn new(buf: Vec<u8>, m: Match) -> Self {
        Self { buf, m }
    }

    pub fn found_match(&self) -> &[u8] {
        &self.buf[self.m.start()..self.m.end()]
    }

    pub fn before_match(&self) -> &[u8] {
        &self.buf[..self.m.start()]
    }
}

// todo: create builder for Session
fn build_command(cmd: &str) -> Result<Command, Error> {
    let mut args = cmd.split_whitespace();
    let bin = args.next().ok_or(Error::CommandParsing)?;

    let mut cmd = Command::new(bin);
    cmd.args(args);

    Ok(cmd)
}
