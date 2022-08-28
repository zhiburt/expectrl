//! This module contains a [InteractOptions] which allows a castomization of
//! [crate::Session::interact] flow.

use crate::{
    process::{Healthcheck, NonBlocking},
    session::{Proc, Session},
    stream::stdin::Stdin,
    Captures, ControlCode, Error,
};
use std::{
    borrow::Cow,
    collections::HashMap,
    io::{self, Read, Stdout, Write},
};

/// InteractOptions represents options of an interactive session.
pub struct InteractOptions<P, S, R, W, C> {
    escape_character: u8,
    input_filter: Option<FilterFn>,
    output_filter: Option<FilterFn>,
    input_handlers: HashMap<String, ActionFn<Session<P, S>, R, W, C>>,
    #[allow(clippy::type_complexity)]
    output_handlers: Vec<(Box<dyn crate::Needle>, OutputFn<Session<P, S>, R, W, C>)>,
    idle_handler: Option<ActionFn<Session<P, S>, R, W, C>>,
    state: C,
}

type ActionFn<S, R, W, C> = Box<dyn FnMut(Context<'_, S, R, W, C>) -> Result<(), Error>>;

type OutputFn<S, R, W, C> = Box<dyn FnMut(Context<'_, S, R, W, C>, Captures) -> Result<(), Error>>;

type FilterFn = Box<dyn FnMut(&[u8]) -> Result<Cow<[u8]>, Error>>;

/// Context provides an interface to use a [Session], IO streams
/// and a state.
pub struct Context<'a, S, R, W, C> {
    session: &'a mut S,
    input: &'a mut R,
    output: &'a mut W,
    state: &'a mut C,
}

impl<S, R, W, C> Context<'_, S, R, W, C> {
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

impl<P, S, R, W> Default for InteractOptions<P, S, R, W, ()> {
    fn default() -> Self {
        Self {
            escape_character: Self::default_escape_char(),
            idle_handler: None,
            input_handlers: HashMap::new(),
            output_handlers: Vec::new(),
            input_filter: None,
            output_filter: None,
            state: (),
        }
    }
}

impl<P, S, R, W, C> InteractOptions<P, S, R, W, C> {
    /// State sets state which will be available in callback calls, throught context variable.
    ///
    /// Please beware that it cleans already set list of callbacks.
    /// So you need to call this method BEFORE you specify callbacks.
    ///
    /// Default state type is a unit type `()`.
    pub fn state<C1>(self, state: C1) -> InteractOptions<P, S, R, W, C1> {
        InteractOptions {
            state,
            escape_character: self.escape_character,
            input_filter: self.input_filter,
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

impl<P, S, R, W, C> InteractOptions<P, S, R, W, C> {
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
        F: FnMut(Context<'_, Session<P, S>, R, W, C>) -> Result<(), Error> + 'static,
    {
        let _ = self.input_handlers.insert(input.into(), Box::new(f));
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
        F: FnMut(Context<'_, Session<P, S>, R, W, C>, Captures) -> Result<(), Error> + 'static,
    {
        self.output_handlers.push((Box::new(needle), Box::new(f)));
        self
    }

    /// Puts a handler which will be called on each interaction.
    pub fn on_idle<F>(mut self, f: F) -> Self
    where
        F: FnMut(Context<'_, Session<P, S>, R, W, C>) -> Result<(), Error> + 'static,
    {
        self.idle_handler = Some(Box::new(f));
        self
    }

    fn default_escape_char() -> u8 {
        ControlCode::GroupSeparator.into() // Ctrl-]
    }

    fn check_input(
        &mut self,
        input: &mut R,
        output: &mut W,
        session: &mut Session<P, S>,
        bytes: &[u8],
    ) -> Result<Match, Error> {
        for (pattern, callback) in self.input_handlers.iter_mut() {
            if !pattern.is_empty() && !bytes.is_empty() {
                match contains_in_bytes(bytes, pattern.as_bytes()) {
                    Match::No => {}
                    Match::MaybeLater => {
                        return Ok(Match::MaybeLater);
                    }
                    Match::Yes(n) => {
                        let context = Context {
                            state: &mut self.state,
                            session,
                            input,
                            output,
                        };
                        (callback)(context)?;
                        return Ok(Match::Yes(n));
                    }
                }
            }
        }

        Ok(Match::No)
    }

    fn check_output(
        &mut self,
        input: &mut R,
        output: &mut W,
        session: &mut Session<P, S>,
        buf: &mut Vec<u8>,
        eof: bool,
    ) -> Result<(), Error> {
        'checks: loop {
            for (search, callback) in self.output_handlers.iter_mut() {
                let found = search.check(buf, eof)?;
                if !found.is_empty() {
                    let end_index = Captures::right_most_index(&found);
                    let involved_bytes = buf[..end_index].to_vec();
                    let found = Captures::new(involved_bytes, found);
                    let _ = buf.drain(..end_index);

                    let context = Context {
                        state: &mut self.state,
                        session,
                        input,
                        output,
                    };
                    (callback)(context, found)?;

                    continue 'checks;
                }
            }

            return Ok(());
        }
    }

    fn call_idle_handler(
        &mut self,
        input: &mut R,
        output: &mut W,
        session: &mut Session<P, S>,
    ) -> Result<(), Error> {
        let context = Context {
            state: &mut self.state,
            session,
            input,
            output,
        };
        if let Some(callback) = self.idle_handler.as_mut() {
            (callback)(context)?;
        }

        Ok(())
    }
}

#[cfg(all(not(feature = "async"), not(feature = "polling")))]
impl<P, S, R, W, C> InteractOptions<P, S, R, W, C>
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
    pub fn interact(
        &mut self,
        session: &mut Session<P, S>,
        mut input: R,
        mut output: W,
    ) -> Result<(), Error> {
        interact(self, session, &mut input, &mut output)
    }
}

#[cfg(all(not(feature = "async"), not(feature = "polling")))]
impl<S, C> InteractOptions<Proc, S, Stdin, Stdout, C>
where
    S: NonBlocking + Read + Write,
{
    /// This function runs an interact loop with a terminal, using the provided settings.
    ///
    /// If you don't use any settings you can use [Session::interact].
    pub fn interact_in_terminal(&mut self, session: &mut Session<Proc, S>) -> Result<(), Error> {
        let mut stdin = crate::stream::stdin::Stdin::new()?;
        stdin.blocking(true)?;
        let r = interact(self, session, &mut stdin, &mut std::io::stdout());
        stdin.blocking(false)?;
        stdin.close()?;
        r
    }
}

#[cfg(all(unix, not(feature = "async"), feature = "polling"))]
impl<P, S, R, W, C> InteractOptions<P, S, R, W, C>
where
    P: Healthcheck,
    S: NonBlocking + Read + Write + polling::Source,
    R: Read + polling::Source,
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
    pub fn interact(
        &mut self,
        session: &mut Session<P, S>,
        mut input: R,
        mut output: W,
    ) -> Result<(), Error> {
        interact(self, session, &mut input, &mut output)
    }
}

#[cfg(all(unix, not(feature = "async"), feature = "polling"))]
impl<S, C> InteractOptions<Proc, S, Stdin, Stdout, C>
where
    S: NonBlocking + Read + Write + polling::Source,
{
    /// This function runs an interact loop with a terminal, using the provided settings.
    ///
    /// If you don't use any settings you can use [Session::interact].
    pub fn interact_in_terminal(&mut self, session: &mut Session<Proc, S>) -> Result<(), Error> {
        let mut stdin = crate::stream::stdin::Stdin::new()?;
        let r = interact(self, session, &mut stdin, &mut std::io::stdout());
        stdin.close()?;
        r
    }
}

#[cfg(all(windows, not(feature = "async"), feature = "polling"))]
impl<R, W, C> InteractOptions<crate::session::Proc, crate::session::Stream, R, W, C>
where
    R: Read + Send + Clone + 'static,
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
    pub fn interact(
        &mut self,
        session: &mut Session,
        input: R,
        output: W,
    ) -> Result<(), Error> {
        interact(self, session, input, output)
    }
}

#[cfg(all(windows, not(feature = "async"), feature = "polling"))]
impl<C> InteractOptions<Proc, crate::session::Stream, Stdin, Stdout, C>
{
    /// This function runs an interact loop with a terminal, using the provided settings.
    ///
    /// If you don't use any settings you can use [Session::interact].
    pub fn interact_in_terminal(&mut self, session: &mut Session) -> Result<(), Error> {
        let stdin = Stdin::new()?;
        let r = interact(self, session,  stdin,  std::io::stdout());
        r
    }
}

#[cfg(feature = "async")]
impl<P, S, R, W, C> InteractOptions<P, S, R, W, C>
where
    P: Healthcheck + Unpin,
    S: futures_lite::AsyncRead + futures_lite::AsyncWrite + Unpin,
    R: futures_lite::AsyncRead + Unpin,
    W: Write,
{
    /// Runs interact interactively.
    /// See [Session::interact]
    pub async fn interact(
        &mut self,
        session: &mut Session<P, S>,
        mut input: R,
        mut output: W,
    ) -> Result<(), Error> {
        interact(self, session, &mut input, &mut output).await
    }
}

#[cfg(feature = "async")]
impl<S, C> InteractOptions<Proc, S, Stdin, Stdout, C>
where
    S: futures_lite::AsyncRead + futures_lite::AsyncWrite + Unpin,
{
    /// This function runs an interact loop with a terminal, using the provided settings.
    ///
    /// If you don't use any settings you can use [Session::interact].
    pub async fn interact_in_terminal(
        &mut self,
        session: &mut Session<Proc, S>,
    ) -> Result<(), Error> {
        let mut stdin = crate::stream::stdin::Stdin::new()?;
        let r = interact(self, session, &mut stdin, &mut std::io::stdout()).await;
        stdin.close()?;
        r
    }
}

#[cfg(all(not(feature = "async"), not(feature = "polling")))]
fn interact<P, S, R, W, C>(
    options: &mut InteractOptions<P, S, R, W, C>,
    session: &mut Session<P, S>,
    input: &mut R,
    output: &mut W,
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
        let status = session.is_alive();
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
                options.check_output(input, output, session, &mut output_buffer, eof)?;

                let bytes = if let Some(filter) = options.output_filter.as_mut() {
                    (filter)(&buf[..n])?
                } else {
                    Cow::Borrowed(&buf[..n])
                };

                output.write_all(&bytes)?;
                output.flush()?;
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
        match input.read(&mut buf) {
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
                        match options.check_input(input, output, session, check_buffer)? {
                            Match::Yes(n) => {
                                let _ = check_buffer.drain(..n);
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

        options.call_idle_handler(input, output, session)?;
    }
}

#[cfg(all(unix, not(feature = "async"), feature = "polling"))]
fn interact<P, S, R, W, C>(
    options: &mut InteractOptions<P, S, R, W, C>,
    session: &mut Session<P, S>,
    input: &mut R,
    output: &mut W,
) -> Result<(), Error>
where
    P: Healthcheck,
    S: NonBlocking + Read + Write + polling::Source,
    R: Read + polling::Source,
    W: Write,
{
    use polling::{Event, Poller};

    let mut output_buffer = Vec::new();
    let options_has_input_checks = !options.input_handlers.is_empty();
    let mut input_buffer = if options_has_input_checks {
        Some(Vec::new())
    } else {
        None
    };
    let mut exited = false;

    // Create a poller and register interest in readability on the socket.
    let poller = Poller::new()?;
    poller.add(&input.raw(), Event::readable(0))?;
    poller.add(&session.get_stream().raw(), Event::readable(1))?;

    let mut buf = [0; 512];

    // The event loop.
    let mut events = Vec::new();
    loop {
        // In case where proceses exits we are trying to
        // fill buffer to run callbacks if there was something in.
        //
        // We ignore errors because there might be errors like EOCHILD etc.
        let status = session.is_alive();
        if matches!(status, Ok(false)) {
            exited = true;
        }

        // Wait for at least one I/O event.
        events.clear();
        let _ = poller.wait(&mut events, None)?;

        for ev in &events {
            if ev.key == 0 {
                // We dont't print user input back to the screen.
                // In terminal mode it will be ECHOed back automatically.
                // This way we preserve terminal seetings for example when user inputs password.
                // The terminal must have been prepared before.
                match input.read(&mut buf) {
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
                                match options.check_input(input, output, session, check_buffer)? {
                                    Match::Yes(n) => {
                                        let _ = check_buffer.drain(..n);
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

                // Set interest in the next readability event.
                poller.modify(&input.raw(), Event::readable(0))?;
            }

            if ev.key == 1 {
                match session.try_read(&mut buf) {
                    Ok(n) => {
                        let eof = n == 0;
                        if eof {
                            exited = true;
                        }

                        output_buffer.extend_from_slice(&buf[..n]);
                        options.check_output(input, output, session, &mut output_buffer, eof)?;

                        let bytes = if let Some(filter) = options.output_filter.as_mut() {
                            (filter)(&buf[..n])?
                        } else {
                            Cow::Borrowed(&buf[..n])
                        };

                        output.write_all(&bytes)?;
                        output.flush()?;
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => return Err(err.into()),
                }

                // Set interest in the next readability event.
                poller.modify(&session.get_stream().raw(), Event::readable(1))?;
            }
        }

        if exited {
            return Ok(());
        }

        options.call_idle_handler(input, output, session)?;
    }
}

#[cfg(all(windows, not(feature = "async"), feature = "polling"))]
fn interact<R, W, C>(
    options: &mut InteractOptions<crate::session::Proc, crate::session::Stream, R, W, C>,
    session: &mut crate::session::Session,
    input: R,
    mut output: W,
) -> Result<(), Error>
where
    R: Read + Send + 'static,
    W: Write,
{
    use crate::{waiter::{Recv, Wait2}, error::to_io_error};

    let mut output_buffer = Vec::new();

    // Create a poller and register interest in readability on the socket.
    let stream = session.get_stream().try_clone().map_err(to_io_error(""))?;
    let mut poller = Wait2::new(input, stream);

    loop {
        // In case where proceses exits we are trying to
        // fill buffer to run callbacks if there was something in.
        //
        // We ignore errors because there might be errors like EOCHILD etc.
        let status = session.is_alive();
        if matches!(status, Ok(false)) {
            return Ok(())
        }

        // Wait for at least one I/O event.
        let event = poller.recv().map_err(to_io_error(""))?;

        match event {
            Recv::R1(b) => {
                match b {
                    Ok(None) => {
                        return Ok(());
                    }
                    Ok(Some(b)) => {
                        let bytes: &[u8] = &[b];
                        let buffer = if let Some(filter) = options.input_filter.as_mut() {
                            (filter)(bytes)?
                        } else {
                            Cow::Borrowed(bytes)
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
            },
            Recv::R2(b) => {
                match b {
                    Ok(None) => {
                        return Ok(());
                    }
                    Ok(Some(b)) => {
                        let buf: &[u8] = &[b];
                        output_buffer.extend_from_slice(buf);

                        let bytes = if let Some(filter) = options.output_filter.as_mut() {
                            (filter)(buf)?
                        } else {
                            Cow::Borrowed(buf)
                        };

                        output.write_all(&bytes)?;
                        output.flush()?;
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => return Err(err.into()),
                }
            },
            Recv::Timeout => {},
        }
    }
}

// copy paste of sync version with async await syntax
#[cfg(feature = "async")]
async fn interact<P, S, R, W, C>(
    options: &mut InteractOptions<P, S, R, W, C>,
    session: &mut Session<P, S>,
    input: &mut R,
    output: &mut W,
) -> Result<(), Error>
where
    P: Healthcheck + Unpin,
    S: futures_lite::AsyncRead + futures_lite::AsyncWrite + Unpin,
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
    let mut stdin_buf = [0; 512];
    let mut proc_buf = [0; 512];
    loop {
        // In case where proceses exits we are trying to
        // fill buffer to run callbacks if there was something in.
        //
        // We ignore errors because there might be errors like EOCHILD etc.
        let status = session.is_alive();
        if matches!(status, Ok(false)) {
            return Ok(());
        }

        #[derive(Debug)]
        enum ReadFrom {
            Stdin,
            Process,
            Timeout,
        }

        let read_process = async { (ReadFrom::Process, session.read(&mut proc_buf).await) };
        let read_stdin = async { (ReadFrom::Stdin, input.read(&mut stdin_buf).await) };
        let timeout = async { (ReadFrom::Timeout, async { futures_timer::Delay::new(std::time::Duration::from_secs(5)).await; io::Result::Ok(0)}.await) };


        let read_fut = futures_lite::future::or(read_process, read_stdin);
        let (read_from, result) = futures_lite::future::or(read_fut, timeout).await;

        match read_from {
            ReadFrom::Process => {
                let n = result?;
                let eof = n == 0;

                output_buffer.extend_from_slice(&proc_buf[..n]);
                options.check_output(input, output, session, &mut output_buffer, eof)?;

                let bytes = if let Some(filter) = options.output_filter.as_mut() {
                    (filter)(&proc_buf[..n])?
                } else {
                    Cow::Borrowed(&proc_buf[..n])
                };

                output.write_all(&bytes)?;
                output.flush()?;

                if eof {
                    return Ok(());
                }
            }
            ReadFrom::Stdin => {
                // We dont't print user input back to the screen.
                // In terminal mode it will be ECHOed back automatically.
                // This way we preserve terminal seetings for example when user inputs password.
                // The terminal must have been prepared before.
                match result {
                    Ok(0) => {
                        return Ok(());
                    }
                    Ok(n) => {
                        let bytes = &stdin_buf[..n];
                        let bytes = if let Some(filter) = options.input_filter.as_mut() {
                            (filter)(bytes)?
                        } else {
                            Cow::Borrowed(bytes)
                        };

                        let buffer = if let Some(check_buffer) = input_buffer.as_mut() {
                            check_buffer.extend_from_slice(&bytes);
                            loop {
                                match options.check_input(input, output, session, check_buffer)? {
                                    Match::Yes(n) => {
                                        let _ = check_buffer.drain(..n);
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
            }
            ReadFrom::Timeout => {
                // We need to check whether a process is alive;
                continue;
            }
        }

        options.call_idle_handler(input, output, session)?;
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
