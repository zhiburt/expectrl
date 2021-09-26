//! This module contains a [InteractOptions] which allows a castomization of
//! [crate::Session::interact] flow.

use crate::{session::Session, ControlCode, Error, Found, Needle};
use std::{
    collections::HashMap,
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

#[cfg(not(feature = "async"))]
use std::io::Read;

#[cfg(windows)]
use conpty::console::Console;

/// InteractOptions represents options of an interact session.
pub struct InteractOptions<R, W> {
    input: R,
    output: W,
    input_from: InputFrom,
    escape_character: u8,
    handlers: HashMap<Action, ActionFn>,
}

enum InputFrom {
    Terminal,
    Other,
}

type ActionFn = Box<dyn FnMut(&mut Session) -> Result<(), Error>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Action {
    Input(String),
}

impl InteractOptions<NonBlockingStdin, io::Stdout> {
    /// Constructs a interact options to interact via STDIN.
    ///
    /// Usage [InteractOptions::streamed] directly with [std::io::stdin],
    /// most likely will provide a correct interact processing.
    /// It depends on terminal settings.
    pub fn terminal() -> Result<Self, Error> {
        let input = NonBlockingStdin::new()?;
        Ok(Self {
            input,
            output: io::stdout(),
            input_from: InputFrom::Terminal,
            escape_character: Self::default_escape_char(),
            handlers: HashMap::new(),
        })
    }
}

impl<R, W> InteractOptions<R, W> {
    /// Create interact options with custom input and output streams.
    ///
    /// To construct default terminal session see [InteractOptions::terminal]
    pub fn streamed(input: R, output: W) -> Result<Self, Error> {
        Ok(Self {
            input,
            output,
            input_from: InputFrom::Other,
            escape_character: Self::default_escape_char(),
            handlers: HashMap::new(),
        })
    }
}

impl<R, W> InteractOptions<R, W> {
    /// Sets an escape character after seen which the interact interactions will be stopped
    /// and controll will be returned to a caller process.
    pub fn escape_character(mut self, c: u8) -> Self {
        self.escape_character = c;
        self
    }

    /// Puts a hanlder which will be called when input is seen in users input.
    ///
    /// Be aware that currently async version doesn't take a Session as an argument.
    /// See https://github.com/zhiburt/expectrl/issues/16.
    pub fn on_input<F>(mut self, input: impl Into<String>, f: F) -> Self
    where
        F: FnMut(&mut Session) -> Result<(), Error> + 'static,
    {
        self.handlers
            .insert(Action::Input(input.into()), Box::new(f));
        self
    }

    fn default_escape_char() -> u8 {
        ControlCode::GroupSeparator.into() // Ctrl-]
    }

    fn check_input(&mut self, session: &mut Session, bytes: &mut Vec<u8>) -> Result<(), Error> {
        for (action, callback) in self.handlers.iter_mut() {
            let Action::Input(pattern) = action;

            // reuse Needle code
            let m = Needle::check(&pattern, bytes, false)?;
            if !m.is_empty() {
                let last_index_which_involved = Found::right_most_index(&m);
                bytes.drain(..last_index_which_involved);
                return (callback)(session);
            }
        }

        Ok(())
    }
}

#[cfg(not(feature = "async"))]
impl<R, W> InteractOptions<R, W>
where
    R: Read,
    W: Write,
{
    /// Runs interact interactively.
    /// See [Session::interact]
    #[cfg(unix)]
    pub fn interact(self, session: &mut Session) -> Result<WaitStatus, Error> {
        match self.input_from {
            InputFrom::Terminal => interact_in_terminal(session, self),
            InputFrom::Other => interact(session, self),
        }
    }

    /// Runs interact interactively.
    /// See [Session::interact]
    #[cfg(windows)]
    pub fn interact(self, session: &mut Session) -> Result<(), Error> {
        match self.input_from {
            InputFrom::Terminal => interact_in_terminal(session, self),
            InputFrom::Other => interact(session, self),
        }
    }
}

#[cfg(feature = "async")]
impl<R, W> InteractOptions<R, W>
where
    R: futures_lite::AsyncRead + std::marker::Unpin,
    W: Write,
{
    /// Runs interact interactively.
    /// See [Session::interact]
    pub async fn interact(self, session: &mut Session) -> Result<WaitStatus, Error> {
        match self.input_from {
            InputFrom::Terminal => interact_in_terminal(session, self).await,
            InputFrom::Other => interact(session, self).await,
        }
    }
}

#[cfg(all(unix, not(feature = "async")))]
fn interact_in_terminal<R, W>(
    session: &mut Session,
    options: InteractOptions<R, W>,
) -> Result<WaitStatus, Error>
where
    R: Read,
    W: Write,
{
    // flush buffers
    session.flush()?;

    let origin_pty_echo = session.get_echo()?;
    // tcgetattr issues error if a provided fd is not a tty,
    // but we can work with such input as it may be redirected.
    let origin_stdin_flags = termios::tcgetattr(STDIN_FILENO);

    // verify: possible controlling fd can be stdout and stderr as well?
    // https://stackoverflow.com/questions/35873843/when-setting-terminal-attributes-via-tcsetattrfd-can-fd-be-either-stdout
    let isatty_terminal = isatty(STDIN_FILENO)?;

    if isatty_terminal {
        set_raw(STDIN_FILENO)?;
    }

    session.set_echo(true)?;

    let result = interact(session, options);

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

#[cfg(unix)]
#[cfg(not(feature = "async"))]
fn interact<R, W>(
    session: &mut Session,
    mut options: InteractOptions<R, W>,
) -> Result<WaitStatus, Error>
where
    R: Read,
    W: Write,
{
    let options_has_input_checks = !options.handlers.is_empty();
    let mut buffer_for_check = if options_has_input_checks {
        Some(Vec::new())
    } else {
        None
    };

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
                options.output.write_all(&buf[..n])?;
                options.output.flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        match options.input.read(&mut buf) {
            Ok(0) => return Ok(status),
            Ok(n) => {
                let escape_char_position =
                    buf[..n].iter().position(|c| *c == options.escape_character);
                match escape_char_position {
                    Some(pos) => {
                        session.write_all(&buf[..pos])?;
                        return Ok(status);
                    }
                    None => {
                        session.write_all(&buf[..n])?;
                    }
                }

                // check callbacks
                if options_has_input_checks {
                    buffer_for_check
                        .as_mut()
                        .unwrap()
                        .extend_from_slice(&buf[..n]);
                    options.check_input(session, buffer_for_check.as_mut().unwrap())?;
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }
    }
}

// copy paste of sync version with async await syntax
#[cfg(all(unix, feature = "async"))]
async fn interact_in_terminal<R, W>(
    session: &mut Session,
    options: InteractOptions<R, W>,
) -> Result<WaitStatus, Error>
where
    R: futures_lite::AsyncRead + std::marker::Unpin,
    W: Write,
{
    use futures_lite::AsyncWriteExt;

    // flush buffers
    session.flush().await?;

    let origin_pty_echo = session.get_echo()?;
    // tcgetattr issues error if a provided fd is not a tty,
    // but we can work with such input as it may be redirected.
    let origin_stdin_flags = termios::tcgetattr(STDIN_FILENO);

    // verify: possible controlling fd can be stdout and stderr as well?
    // https://stackoverflow.com/questions/35873843/when-setting-terminal-attributes-via-tcsetattrfd-can-fd-be-either-stdout
    let isatty_terminal = isatty(STDIN_FILENO)?;

    if isatty_terminal {
        set_raw(STDIN_FILENO)?;
    }

    session.set_echo(true)?;

    let result = interact(session, options).await;

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

// copy paste of sync version with async await syntax
#[cfg(all(unix, feature = "async"))]
async fn interact<R, W>(
    session: &mut Session,
    mut options: InteractOptions<R, W>,
) -> Result<WaitStatus, Error>
where
    R: futures_lite::AsyncRead + std::marker::Unpin,
    W: Write,
{
    use futures_lite::{AsyncReadExt, AsyncWriteExt};

    let options_has_input_checks = !options.handlers.is_empty();
    let mut buffer_for_check = if options_has_input_checks {
        Some(Vec::new())
    } else {
        None
    };

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
                options.output.write_all(&buf[..n])?;
                options.output.flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        match options.input.read(&mut buf).await {
            Ok(0) => return Ok(status),
            Ok(n) => {
                let escape_char_position =
                    buf[..n].iter().position(|c| *c == options.escape_character);
                match escape_char_position {
                    Some(pos) => {
                        session.write_all(&buf[..pos]).await?;
                        return Ok(status);
                    }
                    None => {
                        session.write_all(&buf[..n]).await?;
                    }
                }

                // check callbacks
                if options_has_input_checks {
                    buffer_for_check
                        .as_mut()
                        .unwrap()
                        .extend_from_slice(&buf[..n]);
                    options.check_input(session, buffer_for_check.as_mut().unwrap())?;
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }
    }
}

#[cfg(windows)]
fn interact_in_terminal<R, W>(
    session: &mut Session,
    options: InteractOptions<R, W>,
) -> Result<(), Error>
where
    R: Read,
    W: Write,
{
    // flush buffers
    session.flush()?;

    let console = conpty::console::Console::current()?;
    console.set_raw()?;

    let r = interact(session, options);

    console.reset()?;

    r
}

#[cfg(windows)]
fn interact<R, W>(session: &mut Session, mut options: InteractOptions<R, W>) -> Result<(), Error>
where
    R: Read,
    W: Write,
{
    let options_has_input_checks = !options.handlers.is_empty();
    let mut buffer_for_check = if options_has_input_checks {
        Some(Vec::new())
    } else {
        None
    };

    let mut buf = [0; 512];
    loop {
        match session.try_read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                options.output.write_all(&buf[..n])?;
                options.output.flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        match options.input.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                let escape_char_position =
                    buf[..n].iter().position(|c| *c == options.escape_character);
                match escape_char_position {
                    Some(pos) => {
                        session.write_all(&buf[..pos])?;
                        return Ok(status);
                    }
                    None => {
                        session.write_all(&buf[..n])?;
                    }
                }

                // check callbacks
                if options_has_input_checks {
                    buffer_for_check
                        .as_mut()
                        .unwrap()
                        .extend_from_slice(&buf[..n]);
                    options.check_input(session, buffer_for_check.as_mut().unwrap())?;
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }
    }
}

#[cfg(unix)]
pub struct NonBlockingStdin {
    stream: Stream,
}

#[cfg(unix)]
impl NonBlockingStdin {
    fn new() -> Result<Self, Error> {
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
        let stream = Stream::new(stdin);

        Ok(Self { stream })
    }
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
impl Read for NonBlockingStdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.try_read(buf)
    }
}

#[cfg(unix)]
#[cfg(feature = "async")]
impl futures_lite::AsyncRead for NonBlockingStdin {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<io::Result<usize>> {
        use futures_lite::FutureExt;
        Box::pin(self.stream.try_read(buf)).poll(cx)
    }
}

#[cfg(windows)]
pub struct NonBlockingStdin {
    current_terminal: Console,
}

#[cfg(windows)]
impl NonBlockingStdin {
    fn new() -> Result<Self, Error> {
        let console = conpty::console::Console::current()?;
        Ok(Self {
            current_terminal: console,
        })
    }
}

#[cfg(windows)]
impl Read for NonBlockingStdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // we can't easily read in non-blocking manner,
        // but we can check when there's something to read,
        // which seems to be enough to not block.
        if self.current_terminal.is_stdin_not_empty()? {
            io::stdin().read(&mut buf)
        } else {
            Err(io::Error::new(io::ErrorKind::WouldBlock, ""))
        }
    }
}
