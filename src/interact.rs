use crate::{ControlCode, Error, Session};
use std::{
    fs::File,
    io::{self, Write},
};

#[cfg(unix)]
use crate::{stream::Stream, WaitStatus};
#[cfg(unix)]
use nix::{
    libc::STDIN_FILENO,
    sys::termios,
    unistd::{dup, isatty},
};
#[cfg(unix)]
use ptyprocess::set_raw;
#[cfg(unix)]
use std::os::unix::prelude::FromRawFd;

#[cfg(all(unix, feature = "async"))]
use futures_lite::AsyncWriteExt;

#[cfg(windows)]
use std::io::Read;

pub struct InteractOptions {
    escape_character: u8,
}

impl Default for InteractOptions {
    fn default() -> Self {
        Self {
            escape_character: ControlCode::GroupSeparator.into(), // Ctrl-]
        }
    }
}

impl InteractOptions {
    pub fn escape_character(mut self, c: u8) -> Self {
        self.escape_character = c;
        self
    }

    #[cfg(all(unix, not(feature = "async")))]
    pub fn interact(self, session: &mut Session) -> Result<WaitStatus, Error> {
        interact(session, self.escape_character)
    }

    #[cfg(all(unix, feature = "async"))]
    pub async fn interact(self, session: &mut Session) -> Result<WaitStatus, Error> {
        interact(session, self.escape_character).await
    }

    #[cfg(windows)]
    pub fn interact(self, session: &mut Session) -> Result<(), Error> {
        interact(session, self.escape_character)
    }
}

#[cfg(all(unix, not(feature = "async")))]
fn interact(session: &mut Session, escape_character: u8) -> Result<WaitStatus, Error> {
    // flush buffers
    session.flush()?;

    let origin_pty_echo = session.get_echo()?;
    // tcgetattr issues error if a provided fd is not a tty,
    // but we can work with such input as it may be redirected.
    let origin_stdin_flags = termios::tcgetattr(STDIN_FILENO);

    // verify: possible controlling fd can be stdout and stderr as well?
    // https://stackoverflow.com/questions/35873843/when-setting-terminal-attributes-via-tcsetattrfd-can-fd-be-either-stdout
    let isatty_terminal = isatty(STDIN_FILENO)?;

    // it's crusial to make a DUP call here.
    // If we don't actual stdin will be closed,
    // And any interaction with it may cause errors.
    //
    // Why we don't use a `std::fs::File::try_clone` with a 0 fd?
    // Because for some reason it actually doesn't make the same things as DUP does,
    // eventhough a research showed that it should.
    // https://github.com/zhiburt/expectrl/issues/7#issuecomment-884787229
    let stdin_copy_fd = dup(STDIN_FILENO)?;
    let stdin = unsafe { std::fs::File::from_raw_fd(stdin_copy_fd) };

    if isatty_terminal {
        set_raw(STDIN_FILENO)?;
    }

    session.set_echo(true)?;

    let result = _interact(session, stdin, escape_character);

    if isatty_terminal {
        // it's suppose to be always OK.
        // but we don't use unwrap just in case.
        let origin_stdin_flags = origin_stdin_flags?;

        termios::tcsetattr(
            STDIN_FILENO,
            termios::SetArg::TCSAFLUSH,
            &origin_stdin_flags,
        )?;
    }

    session.set_echo(origin_pty_echo)?;

    result
}

#[cfg(all(unix, not(feature = "async")))]
fn _interact(
    session: &mut Session,
    stdin: File,
    escape_character: u8,
) -> Result<WaitStatus, Error> {
    let mut stdin_stream = Stream::new(stdin);

    let mut buf = [0; 512];
    loop {
        let status = session.status()?;
        if !matches!(status, WaitStatus::StillAlive) {
            return Ok(status);
        }

        // it prints STDIN input as well,
        // by echoing it.
        //
        // the setting must be set before calling the function.
        match session.try_read(&mut buf) {
            Ok(0) => return Ok(status),
            Ok(n) => {
                io::stdout().write_all(&buf[..n])?;
                io::stdout().flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        match stdin_stream.try_read(&mut buf) {
            Ok(0) => return Ok(status),
            Ok(n) => {
                let escape_char_position = buf[..n].iter().position(|c| *c == escape_character);
                match escape_char_position {
                    Some(pos) => {
                        session.write_all(&buf[..pos])?;
                        return Ok(status);
                    }
                    None => {
                        session.write_all(&buf[..n])?;
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }
    }
}

// copy paste of sync version with async await syntax
#[cfg(all(unix, feature = "async"))]
async fn interact(session: &mut Session, escape_character: u8) -> Result<WaitStatus, Error> {
    // flush buffers
    session.flush().await?;

    let origin_pty_echo = session.get_echo()?;
    // tcgetattr issues error if a provided fd is not a tty,
    // but we can work with such input as it may be redirected.
    let origin_stdin_flags = termios::tcgetattr(STDIN_FILENO);

    // verify: possible controlling fd can be stdout and stderr as well?
    // https://stackoverflow.com/questions/35873843/when-setting-terminal-attributes-via-tcsetattrfd-can-fd-be-either-stdout
    let isatty_terminal = isatty(STDIN_FILENO)?;

    // it's crusial to make a DUP call here.
    // If we don't actual stdin will be closed,
    // And any interaction with it may cause errors.
    //
    // Why we don't use a `std::fs::File::try_clone` with a 0 fd?
    // Because for some reason it actually doesn't make the same things as DUP does,
    // eventhough a research showed that it should.
    // https://github.com/zhiburt/expectrl/issues/7#issuecomment-884787229
    let stdin_copy_fd = dup(STDIN_FILENO)?;
    let stdin = unsafe { std::fs::File::from_raw_fd(stdin_copy_fd) };

    if isatty_terminal {
        set_raw(STDIN_FILENO)?;
    }

    session.set_echo(true)?;

    let result = _interact(session, stdin, escape_character).await;

    if isatty_terminal {
        // it's suppose to be always OK.
        // but we don't use unwrap just in case.
        let origin_stdin_flags = origin_stdin_flags?;

        termios::tcsetattr(
            STDIN_FILENO,
            termios::SetArg::TCSAFLUSH,
            &origin_stdin_flags,
        )?;
    }

    session.set_echo(origin_pty_echo)?;

    result
}

#[cfg(all(unix, feature = "async"))]
async fn _interact(
    session: &mut Session,
    stdin: File,
    escape_character: u8,
) -> Result<WaitStatus, Error> {
    let mut stdin_stream = Stream::new(stdin);

    let mut buf = [0; 512];
    loop {
        let status = session.status()?;
        if !matches!(status, WaitStatus::StillAlive) {
            return Ok(status);
        }

        // it prints STDIN input as well,
        // by echoing it.
        //
        // the setting must be set before calling the function.
        match session.try_read(&mut buf).await {
            Ok(0) => return Ok(status),
            Ok(n) => {
                io::stdout().write_all(&buf[..n])?;
                io::stdout().flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        match stdin_stream.try_read(&mut buf).await {
            Ok(0) => return Ok(status),
            Ok(n) => {
                let escape_char_position = buf[..n].iter().position(|c| *c == escape_character);
                match escape_char_position {
                    Some(pos) => {
                        session.write_all(&buf[..pos]).await?;
                        return Ok(status);
                    }
                    None => {
                        session.write_all(&buf[..n]).await?;
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }
    }
}

#[cfg(windows)]
fn interact(session: &mut Session, escape_character: u8) -> Result<(), Error> {
    // flush buffers
    self.flush()?;

    let console = conpty::console::Console::current().unwrap();
    console.set_raw().unwrap();

    let r = self._interact(&console, escape_character);

    console.reset().unwrap();

    r
}

#[cfg(windows)]
fn _interact(
    &mut self,
    console: &conpty::console::Console,
    escape_character: u8,
) -> Result<(), Error> {
    let mut buf = [0; 512];
    loop {
        if !self.is_alive() {
            return Ok(());
        }

        match self.try_read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                io::stdout().write_all(&buf[..n])?;
                io::stdout().flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err),
        }

        // we can't easily read in non-blocking manner,
        // but we can check when there's something to read,
        // which seems to be enough to not block.
        if console.is_stdin_not_empty()? {
            let n = io::stdin().read(&mut buf)?;
            if n == 0 {
                return Ok(());
            }

            let escape_char_position = buf[..n].iter().position(|c| *c == escape_character);
            match escape_char_position {
                Some(pos) => {
                    session.write_all(&buf[..pos])?;
                    return Ok(());
                }
                None => {
                    session.write_all(&buf[..n])?;
                }
            }
        }
    }
}
