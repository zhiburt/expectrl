//! This module contains a [InteractOptions] which allows a castomization of
//! [crate::Session::interact] flow.

use crate::{session::Session, ControlCode, Error};
use std::{
    borrow::Cow,
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
    inpput_handlers: HashMap<String, ActionFn>,
    output_handler: Option<OutputFn>,
    idle_handler: Option<ActionFn>,
    input_filter: Option<FilterFn>,
    output_filter: Option<FilterFn>,
}

enum InputFrom {
    Terminal,
    Other,
}

type ActionFn = Box<dyn FnMut(&mut Session) -> Result<(), Error>>;

type OutputFn = Box<dyn FnMut(&mut Session, &[u8]) -> Result<(), Error>>;

type FilterFn = Box<dyn FnMut(&[u8]) -> Result<Cow<[u8]>, Error>>;

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
            inpput_handlers: HashMap::new(),
            idle_handler: None,
            output_handler: None,
            input_filter: None,
            output_filter: None,
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
            inpput_handlers: HashMap::new(),
            idle_handler: None,
            output_handler: None,
            input_filter: None,
            output_filter: None,
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

    /// Sets the output filter.
    /// The output_filter will be passed all the output from the child process.
    ///
    /// The filter is called BEFORE calling a on_output callback if it's set.
    #[cfg(not(feature = "async"))]
    pub fn output_filter<F>(mut self, f: F) -> Self
    where
        F: FnMut(&[u8]) -> Result<Cow<[u8]>, Error> + 'static,
    {
        self.output_filter = Some(Box::new(f));
        self
    }

    /// Sets the input filter.
    /// The input_filter will be passed all the keyboard input from the user.
    ///
    /// The input_filter is run BEFORE the check for the escape_character.
    /// The filter is called BEFORE calling a on_input callback if it's set.
    #[cfg(not(feature = "async"))]
    pub fn input_filter<F>(mut self, f: F) -> Self
    where
        F: FnMut(&[u8]) -> Result<Cow<[u8]>, Error> + 'static,
    {
        self.input_filter = Some(Box::new(f));
        self
    }

    /// Puts a hanlder which will be called when input is seen in users input.
    ///
    /// The matched bytes won't be send to process.
    ///
    /// Be aware that currently async version doesn't take a Session as an argument.
    /// See https://github.com/zhiburt/expectrl/issues/16.
    pub fn on_input<F>(mut self, input: impl Into<String>, f: F) -> Self
    where
        F: FnMut(&mut Session) -> Result<(), Error> + 'static,
    {
        self.inpput_handlers.insert(input.into(), Box::new(f));
        self
    }

    /// Puts a handler which will be called when process produced something in output.
    #[cfg(not(feature = "async"))]
    pub fn on_output<F>(mut self, f: F) -> Self
    where
        F: FnMut(&mut Session, &[u8]) -> Result<(), Error> + 'static,
    {
        self.output_handler = Some(Box::new(f));
        self
    }

    /// Puts a handler which will be called on each interaction.
    #[cfg(not(feature = "async"))]
    pub fn on_idle<F>(mut self, f: F) -> Self
    where
        F: FnMut(&mut Session) -> Result<(), Error> + 'static,
    {
        self.idle_handler = Some(Box::new(f));
        self
    }

    fn default_escape_char() -> u8 {
        ControlCode::GroupSeparator.into() // Ctrl-]
    }

    fn check_input(&mut self, session: &mut Session, bytes: &[u8]) -> Result<Match, Error> {
        for (pattern, callback) in self.inpput_handlers.iter_mut() {
            if !pattern.is_empty() && !bytes.is_empty() {
                match contains_in_bytes(bytes, pattern.as_bytes()) {
                    Match::No => {}
                    Match::MaybeLater => {
                        return Ok(Match::MaybeLater);
                    }
                    Match::Yes(n) => {
                        (callback)(session)?;
                        return Ok(Match::Yes(n));
                    }
                }
            }
        }

        Ok(Match::No)
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
    let options_has_input_checks = !options.inpput_handlers.is_empty();
    let mut input_buffer = if options_has_input_checks {
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

        // In case of terminal
        // it prints STDIN input by echoing it.
        // The terminal must have been prepared before calling the function.
        match session.try_read(&mut buf) {
            Ok(0) => return Ok(status),
            Ok(n) => {
                let bytes = &buf[..n];
                let bytes = if let Some(filter) = options.output_filter.as_mut() {
                    (filter)(bytes)?
                } else {
                    Cow::Borrowed(bytes)
                };

                if let Some(output_callback) = options.output_handler.as_mut() {
                    (output_callback)(session, &bytes)?;
                }

                options.output.write_all(&bytes)?;
                options.output.flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        match options.input.read(&mut buf) {
            Ok(0) => return Ok(status),
            Ok(n) => {
                let bytes = &buf[..n];
                let bytes = if let Some(filter) = options.input_filter.as_mut() {
                    (filter)(bytes)?
                } else {
                    Cow::Borrowed(bytes)
                };

                let buffer = if let Some(check_buffer) = input_buffer.as_mut() {
                    check_buffer.extend_from_slice(&bytes);
                    loop {
                        match options.check_input(session, check_buffer)? {
                            Match::Yes(n) => {
                                check_buffer.drain(..n);
                                if check_buffer.is_empty() {
                                    break vec![];
                                }
                            }
                            Match::No => {
                                let buffer = check_buffer.to_vec();
                                check_buffer.clear();
                                break buffer;
                            }
                            Match::MaybeLater => break vec![],
                        }
                    }
                } else {
                    bytes.to_vec()
                };

                let escape_char_position =
                    buffer.iter().position(|c| *c == options.escape_character);
                match escape_char_position {
                    Some(pos) => {
                        session.write_all(&buffer[..pos])?;
                        return Ok(status);
                    }
                    None => {
                        session.write_all(&buffer[..])?;
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        if let Some(handler) = options.idle_handler.as_mut() {
            (handler)(session)?;
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

    let options_has_input_checks = !options.inpput_handlers.is_empty();
    let mut input_buffer = if options_has_input_checks {
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

        // In case of terminal
        // it prints STDIN input by echoing it.
        // The terminal must have been prepared before calling the function.
        match session.try_read(&mut buf).await {
            Ok(0) => return Ok(status),
            Ok(n) => {
                let bytes = &buf[..n];
                let bytes = if let Some(filter) = options.output_filter.as_mut() {
                    (filter)(bytes)?
                } else {
                    Cow::Borrowed(bytes)
                };

                if let Some(output_callback) = options.output_handler.as_mut() {
                    (output_callback)(session, &bytes)?;
                }

                options.output.write_all(&bytes)?;
                options.output.flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        match options.input.read(&mut buf).await {
            Ok(0) => return Ok(status),
            Ok(n) => {
                let bytes = &buf[..n];
                let bytes = if let Some(filter) = options.input_filter.as_mut() {
                    (filter)(bytes)?
                } else {
                    Cow::Borrowed(bytes)
                };

                let buffer = if let Some(check_buffer) = input_buffer.as_mut() {
                    check_buffer.extend_from_slice(&bytes);
                    loop {
                        match options.check_input(session, check_buffer)? {
                            Match::Yes(n) => {
                                check_buffer.drain(..n);
                                if check_buffer.is_empty() {
                                    break vec![];
                                }
                            }
                            Match::No => {
                                let buffer = check_buffer.to_vec();
                                check_buffer.clear();
                                break buffer;
                            }
                            Match::MaybeLater => break vec![],
                        }
                    }
                } else {
                    bytes.to_vec()
                };

                let escape_char_position =
                    buffer.iter().position(|c| *c == options.escape_character);
                match escape_char_position {
                    Some(pos) => {
                        session.write_all(&buffer[..pos]).await?;
                        return Ok(status);
                    }
                    None => {
                        session.write_all(&buffer[..]).await?;
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        if let Some(handler) = options.idle_handler.as_mut() {
            (handler)(session)?;
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

// copy paste of unix version with changed return type
#[cfg(windows)]
fn interact<R, W>(session: &mut Session, mut options: InteractOptions<R, W>) -> Result<(), Error>
where
    R: Read,
    W: Write,
{
    let options_has_input_checks = !options.inpput_handlers.is_empty();
    let mut input_buffer = if options_has_input_checks {
        Some(Vec::new())
    } else {
        None
    };

    let mut buf = [0; 512];
    loop {
        if !session.is_alive() {
            return Ok(());
        }

        // In case of terminal
        // it prints STDIN input by echoing it.
        // The terminal must have been prepared before calling the function.
        match session.try_read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                let bytes = &buf[..n];
                let bytes = if let Some(filter) = options.output_filter.as_mut() {
                    (filter)(bytes)?
                } else {
                    Cow::Borrowed(bytes)
                };

                if let Some(output_callback) = options.output_handler.as_mut() {
                    (output_callback)(session, &bytes)?;
                }

                options.output.write_all(&bytes)?;
                options.output.flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        match options.input.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                let bytes = &buf[..n];
                let bytes = if let Some(filter) = options.input_filter.as_mut() {
                    (filter)(bytes)?
                } else {
                    Cow::Borrowed(bytes)
                };

                let buffer = if let Some(check_buffer) = input_buffer.as_mut() {
                    check_buffer.extend_from_slice(&bytes);
                    loop {
                        match options.check_input(session, check_buffer)? {
                            Match::Yes(n) => {
                                check_buffer.drain(..n);
                                if check_buffer.is_empty() {
                                    break vec![];
                                }
                            }
                            Match::No => {
                                let buffer = check_buffer.to_vec();
                                check_buffer.clear();
                                break buffer;
                            }
                            Match::MaybeLater => break vec![],
                        }
                    }
                } else {
                    bytes.to_vec()
                };

                let escape_char_position =
                    buffer.iter().position(|c| *c == options.escape_character);
                match escape_char_position {
                    Some(pos) => {
                        session.write_all(&buffer[..pos])?;
                        return Ok(());
                    }
                    None => {
                        session.write_all(&buffer[..])?;
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        if let Some(handler) = options.idle_handler.as_mut() {
            (handler)(session)?;
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
            io::stdin().read(buf)
        } else {
            Err(io::Error::new(io::ErrorKind::WouldBlock, ""))
        }
    }
}

fn contains_in_bytes(haystack: &[u8], find: &[u8]) -> Match {
    assert!(!find.is_empty() && !haystack.is_empty());

    let len = haystack.len().min(find.len());
    let mut i = 0;
    while haystack[i..].len() >= len {
        if haystack[i..i + len] == find[..len] {
            return if len == find.len() {
                Match::Yes(i + len)
            } else {
                Match::MaybeLater
            };
        }

        i += 1;
    }

    Match::No
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Match {
    Yes(usize),
    No,
    MaybeLater,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_in_bytes_test() {
        assert_eq!(contains_in_bytes(b"123", b"123"), Match::Yes(3));
        assert_eq!(contains_in_bytes(b"12345", b"123"), Match::Yes(3));
        assert_eq!(contains_in_bytes(b"1", b"123"), Match::MaybeLater);
        assert_eq!(contains_in_bytes(b"12", b"123"), Match::MaybeLater);
        assert_eq!(contains_in_bytes(b"4", b"123"), Match::No);
        assert_eq!(contains_in_bytes(b"qwe", b"w"), Match::Yes(2));
        assert_eq!(contains_in_bytes(b"qwe", b"j"), Match::No);
    }
}
