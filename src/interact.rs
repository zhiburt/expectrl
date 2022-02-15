//! This module contains a [InteractOptions] which allows a castomization of
//! [crate::Session::interact] flow.

use crate::{
    process::Healthcheck, session::sync_stream::NonBlocking, session::Session, ControlCode, Error,
};
use std::{
    borrow::Cow,
    collections::HashMap,
    io::{self, Read, Write},
};

/// InteractOptions represents options of an interact session.
pub struct InteractOptions<S, R, W, C> {
    input: R,
    output: W,
    escape_character: u8,
    input_filter: Option<FilterFn>,
    output_filter: Option<FilterFn>,
    input_handlers: HashMap<String, ActionFn<S, R, W, C>>,
    #[allow(clippy::type_complexity)]
    output_handlers: Vec<(Box<dyn crate::Needle>, OutputFn<S, R, W, C>)>,
    idle_handler: Option<ActionFn<S, R, W, C>>,
    state: C,
}

type ActionFn<S, R, W, C> = Box<dyn FnMut(Context<'_, S, R, W, C>) -> Result<(), Error>>;

type OutputFn<S, R, W, C> =
    Box<dyn FnMut(Context<'_, S, R, W, C>, crate::Found) -> Result<(), Error>>;

type FilterFn = Box<dyn FnMut(&[u8]) -> Result<Cow<[u8]>, Error>>;

/// Context provides an interface to use a [Session], IO streams
/// and a state.
pub struct Context<'a, S, R, W, C> {
    session: &'a mut S,
    input: &'a mut R,
    output: &'a mut W,
    state: &'a mut C,
}

impl<'a, S, R, W, C> Context<'a, S, R, W, C> {
    /// Get a reference to the context's session.
    pub fn session(&mut self) -> &mut S {
        self.session
    }

    /// Get a reference to the context's input.
    pub fn input(&mut self) -> &mut R {
        self.input
    }

    /// Get a reference to the context's output.
    pub fn output(&mut self) -> &mut W {
        self.output
    }

    /// Get a reference to the context's state.
    pub fn state(&mut self) -> &mut C {
        self.state
    }
}

impl<S, R, W> InteractOptions<S, R, W, ()> {
    /// Create interact options with custom input and output streams.
    ///
    /// To construct default terminal session see [InteractOptions::terminal]
    pub fn streamed(input: R, output: W) -> Result<Self, Error> {
        Ok(Self {
            input,
            output,
            escape_character: Self::default_escape_char(),
            idle_handler: None,
            input_handlers: HashMap::new(),
            output_handlers: Vec::new(),
            input_filter: None,
            output_filter: None,
            state: (),
        })
    }
}

impl<S, R, W, C> InteractOptions<S, R, W, C> {
    /// State sets state which will be available in callback calls, throught context variable.
    ///
    /// Please beware that it cleans already set list of callbacks.
    /// So you need to call this method BEFORE you specify callbacks.
    ///
    /// Default state type is a unit type `()`.
    pub fn state<C1>(self, state: C1) -> InteractOptions<S, R, W, C1> {
        InteractOptions {
            state,
            escape_character: self.escape_character,
            input: self.input,
            input_filter: self.input_filter,
            output: self.output,
            output_filter: self.output_filter,
            idle_handler: None,
            input_handlers: HashMap::new(),
            output_handlers: Vec::new(),
        }
    }

    /// Get a mut reference on state
    pub fn get_state_mut(&mut self) -> &mut C {
        &mut self.state
    }

    /// Get a reference on state
    pub fn get_state(&self) -> &C {
        &self.state
    }
}

impl<S, R, W, C> InteractOptions<S, R, W, C> {
    /// Sets an escape character after seen which the interact interactions will be stopped
    /// and controll will be returned to a caller process.
    pub fn escape_character(mut self, c: u8) -> Self {
        self.escape_character = c;
        self
    }

    /// Sets the output filter.
    /// The output_filter will be passed all the output from the child process.
    ///
    /// The filter isn't applied to user's `read` calls through the [`Context`] in callbacks.
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
    /// See <https://github.com/zhiburt/expectrl/issues/16>.
    pub fn on_input<F>(mut self, input: impl Into<String>, f: F) -> Self
    where
        F: FnMut(Context<'_, S, R, W, C>) -> Result<(), Error> + 'static,
    {
        self.input_handlers.insert(input.into(), Box::new(f));
        self
    }

    /// Puts a handler which will be called when process produced something in output.
    ///
    /// IMPORTANT:
    /// Please be aware that your use of [Session::expect], [Session::check] and any `read` operation on session
    /// will cause the read bytes not to apeard in the output stream!
    pub fn on_output<N, F>(mut self, needle: N, f: F) -> Self
    where
        N: crate::Needle + 'static,
        F: FnMut(Context<'_, S, R, W, C>, crate::Found) -> Result<(), Error> + 'static,
    {
        self.output_handlers.push((Box::new(needle), Box::new(f)));
        self
    }

    /// Puts a handler which will be called on each interaction.
    pub fn on_idle<F>(mut self, f: F) -> Self
    where
        F: FnMut(Context<'_, S, R, W, C>) -> Result<(), Error> + 'static,
    {
        self.idle_handler = Some(Box::new(f));
        self
    }

    fn default_escape_char() -> u8 {
        ControlCode::GroupSeparator.into() // Ctrl-]
    }

    fn check_input(&mut self, session: &mut S, bytes: &[u8]) -> Result<Match, Error> {
        for (pattern, callback) in self.input_handlers.iter_mut() {
            if !pattern.is_empty() && !bytes.is_empty() {
                match contains_in_bytes(bytes, pattern.as_bytes()) {
                    Match::No => {}
                    Match::MaybeLater => {
                        return Ok(Match::MaybeLater);
                    }
                    Match::Yes(n) => {
                        let context = Context {
                            input: &mut self.input,
                            output: &mut self.output,
                            state: &mut self.state,
                            session,
                        };
                        (callback)(context)?;
                        return Ok(Match::Yes(n));
                    }
                }
            }
        }

        Ok(Match::No)
    }

    fn check_output(&mut self, session: &mut S, buf: &mut Vec<u8>, eof: bool) -> Result<(), Error> {
        'checks: loop {
            for (search, callback) in self.output_handlers.iter_mut() {
                let found = search.check(buf, eof)?;
                if !found.is_empty() {
                    let end_index = crate::Found::right_most_index(&found);
                    let involved_bytes = buf[..end_index].to_vec();
                    let found = crate::Found::new(involved_bytes, found);
                    buf.drain(..end_index);

                    let context = Context {
                        input: &mut self.input,
                        output: &mut self.output,
                        state: &mut self.state,
                        session,
                    };
                    (callback)(context, found)?;

                    continue 'checks;
                }
            }

            return Ok(());
        }
    }

    fn call_idle_handler(&mut self, session: &mut S) -> Result<(), Error> {
        let context = Context {
            input: &mut self.input,
            output: &mut self.output,
            state: &mut self.state,
            session,
        };
        if let Some(callback) = self.idle_handler.as_mut() {
            (callback)(context)?;
        }

        Ok(())
    }
}

#[cfg(not(feature = "async"))]
impl<P, S, R, W, C> InteractOptions<Session<P, S>, R, W, C>
where
    P: Healthcheck,
    S: NonBlocking + Read + Write,
    R: Read,
    W: Write,
{
    /// Runs interact interactively.
    /// See [Session::interact]
    ///
    /// On process exit it tries to read available bytes from output in order to run callbacks.
    /// But it is not guaranteed that all output will be read therefore some callbacks might be not called.
    ///
    /// To mitigate such an issue you could use [Session::is_empty] to verify that there is nothing in processes output.
    /// (at the point of the call)
    pub fn interact(&mut self, session: &mut Session<P, S>) -> Result<(), Error> {
        interact(session, self)
    }
}

#[cfg(feature = "async")]
impl<R, W, C> InteractOptions<R, W, C>
where
    R: futures_lite::AsyncRead + std::marker::Unpin,
    W: Write,
{
    /// Runs interact interactively.
    /// See [Session::interact]
    pub async fn interact(self, session: &mut S) -> Result<WaitStatus, Error> {
        interact(session, self)
    }
}

#[cfg(feature = "async")]
impl<R, W, C> InteractOptions<R, W, C>
where
    R: Read + std::marker::Unpin,
    W: Write,
{
    /// Runs interact interactively.
    /// See [Session::interact]
    #[cfg(windows)]
    pub async fn interact(mut self, session: &mut Session) -> Result<(), Error> {
        match self.input_from {
            InputFrom::Terminal => interact_in_terminal(session, &mut self).await,
            InputFrom::Other => interact(session, &mut self).await,
        }
    }
}

#[cfg(not(feature = "async"))]
fn interact<P, S, R, W, C>(
    session: &mut Session<P, S>,
    options: &mut InteractOptions<Session<P, S>, R, W, C>,
) -> Result<(), Error>
where
    P: Healthcheck,
    S: NonBlocking + Read + Write,
    R: Read,
    W: Write,
{
    let mut output_buffer = Vec::new();
    let options_has_input_checks = !options.input_handlers.is_empty();
    let mut input_buffer = if options_has_input_checks {
        Some(Vec::new())
    } else {
        None
    };
    let mut exited = false;

    let mut buf = [0; 512];
    loop {
        // In case where proceses exits we are trying to
        // fill buffer to run callbacks if there was something in.
        //
        // We ignore errors because there might be errors like EOCHILD etc.
        let status = session
            .is_alive()
            .map_err(|e| Error::Other(format!("failed to call status {}", e)));
        if matches!(status, Ok(false)) {
            exited = true;
        }

        match session.try_read(&mut buf) {
            Ok(n) => {
                let eof = n == 0;
                if eof {
                    exited = true;
                }

                output_buffer.extend_from_slice(&buf[..n]);
                options.check_output(session, &mut output_buffer, eof)?;

                let bytes = if let Some(filter) = options.output_filter.as_mut() {
                    (filter)(&buf[..n])?
                } else {
                    Cow::Borrowed(&buf[..n])
                };

                options.output.write_all(&bytes)?;
                options.output.flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        if exited {
            return Ok(());
        }

        // We dont't print user input back to the screen.
        // In terminal mode it will be ECHOed back automatically.
        // This way we preserve terminal seetings for example when user inputs password.
        // The terminal must have been prepared before.
        match options.input.read(&mut buf) {
            Ok(0) => {
                return Ok(());
            }
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

        options.call_idle_handler(session)?;
    }
}

// copy paste of sync version with async await syntax
#[cfg(all(unix, feature = "async"))]
async fn interact_in_terminal<R, W, C>(
    session: &mut Session,
    options: InteractOptions<R, W, C>,
) -> Result<WaitStatus, Error>
where
    R: futures_lite::AsyncRead + std::marker::Unpin,
    W: Write,
{
    use futures_lite::AsyncWriteExt;

    // flush buffers
    session.flush().await?;

    let origin_pty_echo = session.get_echo().map_err(to_io_error)?;
    // tcgetattr issues error if a provided fd is not a tty,
    // but we can work with such input as it may be redirected.
    let origin_stdin_flags = termios::tcgetattr(STDIN_FILENO);

    // verify: possible controlling fd can be stdout and stderr as well?
    // https://stackoverflow.com/questions/35873843/when-setting-terminal-attributes-via-tcsetattrfd-can-fd-be-either-stdout
    let isatty_terminal = isatty(STDIN_FILENO).map_err(to_io_error)?;

    if isatty_terminal {
        set_raw(STDIN_FILENO).map_err(to_io_error)?;
    }

    session.set_echo(true, None).map_err(to_io_error)?;

    let result = interact(session, options).await;

    if isatty_terminal {
        // it's suppose to be always OK.
        // but we don't use unwrap just in case.
        let origin_stdin_flags = origin_stdin_flags.map_err(to_io_error)?;

        termios::tcsetattr(
            STDIN_FILENO,
            termios::SetArg::TCSAFLUSH,
            &origin_stdin_flags,
        )
        .map_err(to_io_error)?;
    }

    session
        .set_echo(origin_pty_echo, None)
        .map_err(to_io_error)?;

    result
}

// copy paste of sync version with async await syntax
#[cfg(all(unix, feature = "async"))]
async fn interact<R, W, C>(
    session: &mut Session,
    mut options: InteractOptions<R, W, C>,
) -> Result<WaitStatus, Error>
where
    R: futures_lite::AsyncRead + Unpin,
    W: Write,
{
    use futures_lite::{AsyncReadExt, AsyncWriteExt};

    let mut output_buffer = Vec::new();
    let options_has_input_checks = !options.input_handlers.is_empty();
    let mut input_buffer = if options_has_input_checks {
        Some(Vec::new())
    } else {
        None
    };
    let mut exited = false;

    let mut buf = [0; 512];
    loop {
        // In case where proceses exits we are trying to
        // fill buffer to run callbacks if there was something in.
        //
        // We ignore errors because there might be errors like EOCHILD etc.
        let status = session.status().map_err(to_io_error).map_err(|e| e.into());
        if !matches!(status, Ok(WaitStatus::StillAlive)) {
            exited = true;
        }

        if let Some(result) = futures_lite::future::poll_once(session.read(&mut buf)).await {
            let n = result?;
            let eof = n == 0;
            if eof {
                exited = true;
            }

            output_buffer.extend_from_slice(&buf[..n]);
            options.check_output(session, &mut output_buffer, eof)?;

            let bytes = if let Some(filter) = options.output_filter.as_mut() {
                (filter)(&buf[..n])?
            } else {
                Cow::Borrowed(&buf[..n])
            };

            options.output.write_all(&bytes)?;
            options.output.flush()?;
        }

        if exited {
            return status;
        }

        // We dont't print user input back to the screen.
        // In terminal mode it will be ECHOed back automatically.
        // This way we preserve terminal seetings for example when user inputs password.
        // The terminal must have been prepared before.
        match options.input.read(&mut buf).await {
            Ok(0) => {
                return status;
            }
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
                        return status;
                    }
                    None => {
                        session.write_all(&buffer[..]).await?;
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        options.call_idle_handler(session)?;
    }
}

#[cfg(windows)]
#[cfg(not(feature = "async"))]
fn interact_in_terminal<R, W, C>(
    session: &mut Session,
    options: &mut InteractOptions<R, W, C>,
) -> Result<(), Error>
where
    R: Read,
    W: Write,
{
    // flush buffers
    session.flush().map_err(to_io_error)?;

    let console = conpty::console::Console::current().map_err(to_io_error)?;
    console.set_raw().map_err(to_io_error)?;

    let r = interact(session, options);

    console.reset().map_err(to_io_error)?;

    r
}

// copy paste of unix version with changed return type
#[cfg(windows)]
#[cfg(not(feature = "async"))]
fn interact<R, W, C>(
    session: &mut Session,
    options: &mut InteractOptions<R, W, C>,
) -> Result<(), Error>
where
    R: Read,
    W: Write,
{
    let options_has_input_checks = !options.input_handlers.is_empty();
    let mut input_buffer = if options_has_input_checks {
        Some(Vec::new())
    } else {
        None
    };

    let mut output_buffer = Vec::new();

    let mut buf = [0; 512];
    loop {
        match session.try_read(&mut buf) {
            Ok(n) => {
                let eof = n == 0;

                output_buffer.extend_from_slice(&buf[..n]);
                options.check_output(session, &mut output_buffer, eof)?;

                if n == 0 {
                    return Ok(());
                }

                let bytes = if let Some(filter) = options.output_filter.as_mut() {
                    (filter)(&buf[..n])?
                } else {
                    Cow::Borrowed(&buf[..n])
                };

                options.output.write_all(&bytes)?;
                options.output.flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        // We dont't print user input back to the screen.
        // In terminal mode it will be ECHOed back automatically.
        // This way we preserve terminal seetings for example when user inputs password.
        // The terminal must have been prepared before.
        match options.input.read(&mut buf) {
            Ok(0) => {
                return Ok(());
            }
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

        options.call_idle_handler(session)?;
    }
}

#[cfg(windows)]
#[cfg(feature = "async")]
async fn interact_in_terminal<R, W, C>(
    session: &mut Session,
    options: &mut InteractOptions<R, W, C>,
) -> Result<(), Error>
where
    R: Read,
    W: Write,
{
    use futures_lite::AsyncWriteExt;

    // flush buffers
    session.flush().await?;

    let console = conpty::console::Console::current().map_err(to_io_error)?;
    console.set_raw().map_err(to_io_error)?;

    let r = interact(session, options).await;

    console.reset().map_err(to_io_error)?;

    r
}

// copy paste of unix version with changed return type
#[cfg(all(windows, feature = "async"))]
async fn interact<R, W, C>(
    session: &mut Session,
    options: &mut InteractOptions<R, W, C>,
) -> Result<(), Error>
where
    R: Read,
    W: Write,
{
    use futures_lite::{AsyncReadExt, AsyncWriteExt};

    let options_has_input_checks = !options.input_handlers.is_empty();
    let mut input_buffer = if options_has_input_checks {
        Some(Vec::new())
    } else {
        None
    };

    let mut output_buffer = Vec::new();

    let mut buf = [0; 512];
    loop {
        match futures_lite::future::poll_once(session.read(&mut buf)).await {
            Some(Ok(n)) => {
                let eof = n == 0;

                output_buffer.extend_from_slice(&buf[..n]);
                options.check_output(session, &mut output_buffer, eof)?;

                if n == 0 {
                    return Ok(());
                }

                let bytes = if let Some(filter) = options.output_filter.as_mut() {
                    (filter)(&buf[..n])?
                } else {
                    Cow::Borrowed(&buf[..n])
                };

                options.output.write_all(&bytes)?;
                options.output.flush()?;
            }
            Some(Err(err)) => return Err(err.into()),
            None => {}
        }

        // We dont't print user input back to the screen.
        // In terminal mode it will be ECHOed back automatically.
        // This way we preserve terminal seetings for example when user inputs password.
        // The terminal must have been prepared before.
        match options.input.read(&mut buf) {
            Ok(0) => {
                return Ok(());
            }
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
                        return Ok(());
                    }
                    None => {
                        session.write_all(&buffer[..]).await?;
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        options.call_idle_handler(session)?;
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

fn to_io_error(err: impl std::error::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err.to_string())
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
