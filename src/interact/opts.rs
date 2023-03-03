#![allow(clippy::type_complexity)]

//! This module contains a [`InteractSession`] which runs an interact session with IO.

use std::{borrow::Cow, io::Write};

use crate::{process::Healthcheck, session::Proc, ControlCode, Error, Session};

#[cfg(not(feature = "async"))]
use std::io::{self, Read};

#[cfg(all(not(feature = "async"), not(feature = "polling")))]
use crate::process::NonBlocking;

use super::Context;

/// InteractConfig represents options of an interactive session.
#[derive(Debug)]
pub struct InteractSession<
    'a,
    State,
    Session,
    Output,
    Input,
    InputFilter,
    OutputFilter,
    InputAction,
    OutputAction,
    IdleAction,
> {
    pub(crate) session: &'a mut Session,
    pub(crate) output: Output,
    pub(crate) input: Input,
    pub(crate) state: State,
    pub(crate) escape_character: u8,
    pub(crate) input_filter: Option<InputFilter>,
    pub(crate) output_filter: Option<OutputFilter>,
    pub(crate) input_action: Option<InputAction>,
    pub(crate) output_action: Option<OutputAction>,
    pub(crate) idle_action: Option<IdleAction>,
}

impl<'a, State, Session, Output, Input>
    InteractSession<
        'a,
        State,
        Session,
        Output,
        Input,
        NoFilter,
        NoFilter,
        NoAction<Session, Output, State>,
        NoAction<Session, Output, State>,
        NoAction<Session, Output, State>,
    >
{
    /// Creates a new object of [InteractSession].
    pub(crate) fn new(
        session: &'a mut Session,
        output: Output,
        input: Input,
        state: State,
    ) -> InteractSession<
        '_,
        State,
        Session,
        Output,
        Input,
        NoFilter,
        NoFilter,
        NoAction<Session, Output, State>,
        NoAction<Session, Output, State>,
        NoAction<Session, Output, State>,
    > {
        InteractSession {
            input,
            output,
            session,
            state,
            escape_character: Self::default_escape_char(),
            input_filter: None,
            output_filter: None,
            input_action: None,
            output_action: None,
            idle_action: None,
        }
    }
}

impl<
        'a,
        State,
        Session,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        InputAction,
        OutputAction,
        IdleAction,
    >
    InteractSession<
        'a,
        State,
        Session,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        InputAction,
        OutputAction,
        IdleAction,
    >
{
    /// Set a state.
    pub fn set_state<S>(
        self,
        state: S,
    ) -> InteractSession<
        'a,
        S,
        Session,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        NoAction<Session, Output, S>,
        NoAction<Session, Output, S>,
        NoAction<Session, Output, S>,
    > {
        InteractSession {
            state,
            input: self.input,
            output: self.output,
            session: self.session,
            escape_character: self.escape_character,
            input_filter: self.input_filter,
            output_filter: self.output_filter,
            input_action: None,
            output_action: None,
            idle_action: None,
        }
    }

    /// Get a reference on state
    pub fn get_state(&self) -> &State {
        &self.state
    }

    /// Get a mut reference on state
    pub fn get_state_mut(&mut self) -> &mut State {
        &mut self.state
    }

    /// Sets an escape character after seen which the interact interactions will be stopped
    /// and controll will be returned to a caller process.
    pub fn set_escape_character(mut self, c: u8) -> Self {
        self.escape_character = c;
        self
    }

    fn default_escape_char() -> u8 {
        ControlCode::GroupSeparator.into() // Ctrl-]
    }

    /// Sets the output filter.
    /// The output_filter will be passed all the output from the child process.
    ///
    /// The filter isn't applied to user's `read` calls through the [`Context`] in callbacks.
    pub fn output_filter<Filter>(
        self,
        f: Filter,
    ) -> InteractSession<
        'a,
        State,
        Session,
        Output,
        Input,
        InputFilter,
        Filter,
        InputAction,
        OutputAction,
        IdleAction,
    >
    where
        Filter: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    {
        InteractSession {
            state: self.state,
            input: self.input,
            output: self.output,
            session: self.session,
            escape_character: self.escape_character,
            input_filter: self.input_filter,
            output_filter: Some(f),
            input_action: self.input_action,
            output_action: self.output_action,
            idle_action: self.idle_action,
        }
    }

    /// Sets the input filter.
    /// The input_filter will be passed all the keyboard input from the user.
    ///
    /// The input_filter is run BEFORE the check for the escape_character.
    /// The filter is called BEFORE calling a on_input callback if it's set.
    pub fn input_filter<Filter>(
        self,
        f: Filter,
    ) -> InteractSession<
        'a,
        State,
        Session,
        Output,
        Input,
        Filter,
        OutputFilter,
        InputAction,
        OutputAction,
        IdleAction,
    >
    where
        Filter: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    {
        InteractSession {
            state: self.state,
            input: self.input,
            output: self.output,
            session: self.session,
            escape_character: self.escape_character,
            input_filter: Some(f),
            output_filter: self.output_filter,
            input_action: self.input_action,
            output_action: self.output_action,
            idle_action: self.idle_action,
        }
    }

    /// Puts a hanlder which will be called when users input is detected.
    ///
    /// Be aware that currently async version doesn't take a Session as an argument.
    /// See <https://github.com/zhiburt/expectrl/issues/16>.
    pub fn on_input<Action>(
        self,
        f: Action,
    ) -> InteractSession<
        'a,
        State,
        Session,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        Action,
        OutputAction,
        IdleAction,
    >
    where
        Action:
            for<'b> FnMut(Context<'b, &mut Session, &mut Output, &mut State>) -> Result<(), Error>,
    {
        InteractSession {
            state: self.state,
            input: self.input,
            output: self.output,
            session: self.session,
            escape_character: self.escape_character,
            input_filter: self.input_filter,
            output_filter: self.output_filter,
            input_action: Some(f),
            output_action: self.output_action,
            idle_action: self.idle_action,
        }
    }

    /// Puts a hanlder which will be called when process output is detected.
    ///
    /// IMPORTANT:
    ///
    /// Please be aware that your use of [Session::expect], [Session::check] and any `read` operation on session
    /// will cause the read bytes not to apeard in the output stream!
    pub fn on_output<Action>(
        self,
        f: Action,
    ) -> InteractSession<
        'a,
        State,
        Session,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        InputAction,
        Action,
        IdleAction,
    >
    where
        Action:
            for<'b> FnMut(Context<'b, &mut Session, &mut Output, &mut State>) -> Result<(), Error>,
    {
        InteractSession {
            state: self.state,
            input: self.input,
            output: self.output,
            session: self.session,
            escape_character: self.escape_character,
            input_filter: self.input_filter,
            output_filter: self.output_filter,
            input_action: self.input_action,
            output_action: Some(f),
            idle_action: self.idle_action,
        }
    }

    /// Puts a handler which will be called on each interaction when no input is detected.
    pub fn on_idle<Action>(
        self,
        f: Action,
    ) -> InteractSession<
        'a,
        State,
        Session,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        InputAction,
        OutputAction,
        Action,
    >
    where
        Action:
            for<'b> FnMut(Context<'b, &mut Session, &mut Output, &mut State>) -> Result<(), Error>,
    {
        InteractSession {
            state: self.state,
            input: self.input,
            output: self.output,
            session: self.session,
            escape_character: self.escape_character,
            input_filter: self.input_filter,
            output_filter: self.output_filter,
            input_action: self.input_action,
            output_action: self.output_action,
            idle_action: Some(f),
        }
    }
}

/// A helper type to set a default action to [`InteractSession`].
pub type NoAction<Session, Output, State> =
    fn(Context<'_, &mut Session, &mut Output, &mut State>) -> Result<(), Error>;

/// A helper type to set a default filter to [`InteractSession`].
pub type NoFilter = fn(&[u8]) -> Result<Cow<'_, [u8]>, Error>;

impl<
        State,
        Stream,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        InputAction,
        OutputAction,
        IdleAction,
    >
    InteractSession<
        '_,
        State,
        Session<Proc, Stream>,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        InputAction,
        OutputAction,
        IdleAction,
    >
{
    /// Runs the session.
    ///
    /// See [`Session::interact`].
    ///
    /// [`Session::interact`]: crate::session::Session::interact
    #[cfg(not(any(feature = "async", feature = "polling")))]
    pub fn spawn(mut self) -> Result<State, Error>
    where
        Stream: NonBlocking + Write + Read,
        Input: Read,
        Output: Write,
        InputFilter: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
        OutputFilter: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
        InputAction: FnMut(
            Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
        ) -> Result<(), Error>,
        OutputAction: FnMut(
            Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
        ) -> Result<(), Error>,
        IdleAction: FnMut(
            Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
        ) -> Result<(), Error>,
    {
        #[cfg(unix)]
        {
            let is_echo = self
                .session
                .get_echo()
                .map_err(|e| Error::unknown("failed to get echo", e.to_string()))?;
            if !is_echo {
                let _ = self.session.set_echo(true, None);
            }

            interact_buzy_loop(&mut self)?;

            if !is_echo {
                let _ = self.session.set_echo(false, None);
            }
        }

        #[cfg(windows)]
        {
            interact_buzy_loop(&mut self)?;
        }

        Ok(self.state)
    }

    /// Runs the session.
    ///
    /// See [`Session::interact`].
    ///
    /// [`Session::interact`]: crate::session::Session::interact
    #[cfg(all(unix, feature = "polling", not(feature = "async")))]
    pub fn spawn(mut self) -> Result<State, Error>
    where
        Stream: Write + Read + std::os::unix::io::AsRawFd,
        Input: Read + std::os::unix::io::AsRawFd,
        Output: Write,
        InputFilter: FnMut(&[u8]) -> Result<Cow<[u8]>, Error>,
        OutputFilter: FnMut(&[u8]) -> Result<Cow<[u8]>, Error>,
        InputAction: FnMut(
            Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
        ) -> Result<(), Error>,
        OutputAction: FnMut(
            Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
        ) -> Result<(), Error>,
        IdleAction: FnMut(
            Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
        ) -> Result<(), Error>,
    {
        let is_echo = self
            .session
            .get_echo()
            .map_err(|e| Error::unknown("failed to get echo", e))?;
        if !is_echo {
            let _ = self.session.set_echo(true, None);
        }

        interact_polling(&mut self)?;

        if !is_echo {
            let _ = self.session.set_echo(false, None);
        }

        Ok(self.state)
    }

    /// Runs the session.
    ///
    /// See [`Session::interact`].
    ///
    /// [`Session::interact`]: crate::session::Session::interact
    #[cfg(feature = "async")]
    pub async fn spawn(mut self) -> Result<State, Error>
    where
        Stream: futures_lite::AsyncRead + futures_lite::AsyncWrite + Unpin,
        Input: futures_lite::AsyncRead + Unpin,
        Output: Write,
        InputFilter: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
        OutputFilter: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
        InputAction: FnMut(
            Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
        ) -> Result<(), Error>,
        OutputAction: FnMut(
            Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
        ) -> Result<(), Error>,
        IdleAction: FnMut(
            Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
        ) -> Result<(), Error>,
    {
        {
            #[cfg(unix)]
            {
                let is_echo = self
                    .session
                    .get_echo()
                    .map_err(|e| Error::unknown("failed to get echo", e.to_string()))?;
                if !is_echo {
                    let _ = self.session.set_echo(true, None);
                }

                interact_async(&mut self).await?;

                if !is_echo {
                    let _ = self.session.set_echo(false, None);
                }
            }

            #[cfg(windows)]
            {
                interact_async(&mut self).await?;
            }

            Ok(self.state)
        }
    }
}

impl<State, Output, Input, InputFilter, OutputFilter, InputAction, OutputAction, IdleAction>
    InteractSession<
        '_,
        State,
        Session,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        InputAction,
        OutputAction,
        IdleAction,
    >
{
    /// Runs the session.
    ///
    /// See [`Session::interact`].
    ///
    /// [`Session::interact`]: crate::session::Session::interact
    #[cfg(all(windows, feature = "polling", not(feature = "async")))]
    pub fn spawn(mut self) -> Result<State, Error>
    where
        Input: Read + Send + 'static,
        Output: Write,
        InputFilter: FnMut(&[u8]) -> Result<Cow<[u8]>, Error>,
        OutputFilter: FnMut(&[u8]) -> Result<Cow<[u8]>, Error>,
        InputAction: FnMut(Context<'_, &mut Session, &mut Output, &mut State>) -> Result<(), Error>,
        OutputAction:
            FnMut(Context<'_, &mut Session, &mut Output, &mut State>) -> Result<(), Error>,
        IdleAction: FnMut(Context<'_, &mut Session, &mut Output, &mut State>) -> Result<(), Error>,
    {
        interact_polling_on_thread(
            self.session,
            self.output,
            self.input,
            &mut self.state,
            self.escape_character,
            self.input_filter,
            self.output_filter,
            self.input_action,
            self.output_action,
            self.idle_action,
        )?;

        Ok(self.state)
    }
}

#[cfg(all(not(feature = "async"), not(feature = "polling")))]
fn interact_buzy_loop<
    State,
    Stream,
    Output,
    Input,
    InputFilter,
    OutputFilter,
    InputAction,
    OutputAction,
    IdleAction,
>(
    opts: &mut InteractSession<
        '_,
        State,
        Session<Proc, Stream>,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        InputAction,
        OutputAction,
        IdleAction,
    >,
) -> Result<(), Error>
where
    Stream: NonBlocking + Write + Read,
    Input: Read,
    Output: Write,
    InputFilter: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    OutputFilter: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    InputAction: FnMut(
        Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
    ) -> Result<(), Error>,
    OutputAction: FnMut(
        Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
    ) -> Result<(), Error>,
    IdleAction: FnMut(
        Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
    ) -> Result<(), Error>,
{
    let mut buf = [0; 512];
    loop {
        // In case where proceses exits we are trying to
        // fill buffer to run callbacks if there was something in.
        //
        // We ignore errors because there might be errors like EOCHILD etc.
        let status = opts.session.is_alive();
        if matches!(status, Ok(false)) {
            return Ok(());
        }

        match opts.session.try_read(&mut buf) {
            Ok(n) => {
                let buf = &buf[..n];
                let buf = if let Some(filter) = opts.output_filter.as_mut() {
                    (filter)(buf)?
                } else {
                    Cow::Borrowed(buf)
                };

                let eof = n == 0;

                if let Some(action) = opts.output_action.as_mut() {
                    let ctx = Context::new(
                        &mut *opts.session,
                        &mut opts.output,
                        &buf,
                        eof,
                        &mut opts.state,
                    );
                    (action)(ctx)?;
                }

                if eof {
                    return Ok(());
                }

                spin_write(&mut opts.output, &buf)?;
                spin_flush(&mut opts.output)?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        // We dont't print user input back to the screen.
        // In terminal mode it will be ECHOed back automatically.
        // This way we preserve terminal seetings for example when user inputs password.
        // The terminal must have been prepared before.
        match opts.input.read(&mut buf) {
            Ok(n) => {
                let buf = &buf[..n];
                let buf = if let Some(filter) = opts.input_filter.as_mut() {
                    (filter)(buf)?
                } else {
                    Cow::Borrowed(buf)
                };

                let eof = n == 0;

                if let Some(action) = opts.input_action.as_mut() {
                    let ctx = Context::new(
                        &mut *opts.session,
                        &mut opts.output,
                        &buf,
                        eof,
                        &mut opts.state,
                    );
                    (action)(ctx)?;
                }

                if eof {
                    return Ok(());
                }

                let escape_char_position = buf.iter().position(|c| *c == opts.escape_character);
                match escape_char_position {
                    Some(pos) => {
                        opts.session.write_all(&buf[..pos])?;
                        return Ok(());
                    }
                    None => {
                        opts.session.write_all(&buf[..])?;
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        if let Some(action) = opts.idle_action.as_mut() {
            let ctx = Context::new(
                &mut *opts.session,
                &mut opts.output,
                &[],
                false,
                &mut opts.state,
            );
            (action)(ctx)?;
        }
    }
}

#[cfg(all(unix, not(feature = "async"), feature = "polling"))]
fn interact_polling<
    State,
    Stream,
    Output,
    Input,
    InputFilter,
    OutputFilter,
    InputAction,
    OutputAction,
    IdleAction,
>(
    opts: &mut InteractSession<
        State,
        Session<Proc, Stream>,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        InputAction,
        OutputAction,
        IdleAction,
    >,
) -> Result<(), Error>
where
    Stream: Write + Read + std::os::unix::io::AsRawFd,
    Input: Read + std::os::unix::io::AsRawFd,
    Output: Write,
    InputFilter: FnMut(&[u8]) -> Result<Cow<[u8]>, Error>,
    OutputFilter: FnMut(&[u8]) -> Result<Cow<[u8]>, Error>,
    InputAction: FnMut(
        Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
    ) -> Result<(), Error>,
    OutputAction: FnMut(
        Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
    ) -> Result<(), Error>,
    IdleAction: FnMut(
        Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
    ) -> Result<(), Error>,
{
    use polling::{Event, Poller};

    // Create a poller and register interest in readability on the socket.
    let poller = Poller::new()?;
    poller.add(&opts.input.as_raw_fd(), Event::readable(0))?;
    poller.add(&opts.session.get_stream().as_raw_fd(), Event::readable(1))?;

    let mut buf = [0; 512];

    // The event loop.
    let mut events = Vec::new();
    loop {
        // In case where proceses exits we are trying to
        // fill buffer to run callbacks if there was something in.
        //
        // We ignore errors because there might be errors like EOCHILD etc.
        let status = opts.session.is_alive();
        if matches!(status, Ok(false)) {
            return Ok(());
        }

        // Wait for at least one I/O event.
        events.clear();
        let _ = poller.wait(&mut events, Some(std::time::Duration::from_secs(5)))?;

        for ev in &events {
            if ev.key == 0 {
                // We dont't print user input back to the screen.
                // In terminal mode it will be ECHOed back automatically.
                // This way we preserve terminal seetings for example when user inputs password.
                // The terminal must have been prepared before.
                match opts.input.read(&mut buf) {
                    Ok(n) => {
                        let buf = &buf[..n];
                        let buf = if let Some(filter) = opts.input_filter.as_mut() {
                            (filter)(buf)?
                        } else {
                            Cow::Borrowed(buf)
                        };

                        let eof = n == 0;

                        if let Some(action) = opts.input_action.as_mut() {
                            let ctx = Context::new(
                                &mut *opts.session,
                                &mut opts.output,
                                &buf,
                                eof,
                                &mut opts.state,
                            );
                            (action)(ctx)?;
                        }

                        if eof {
                            return Ok(());
                        }

                        let escape_char_pos = buf.iter().position(|c| *c == opts.escape_character);
                        match escape_char_pos {
                            Some(pos) => {
                                return opts.session.write_all(&buf[..pos]).map_err(|e| e.into())
                            }
                            None => opts.session.write_all(&buf[..])?,
                        }
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => return Err(err.into()),
                }

                // Set interest in the next readability event.
                poller.modify(&opts.input.as_raw_fd(), Event::readable(0))?;
            }

            if ev.key == 1 {
                match opts.session.read(&mut buf) {
                    Ok(n) => {
                        let buf = &buf[..n];
                        let buf = if let Some(filter) = opts.output_filter.as_mut() {
                            (filter)(buf)?
                        } else {
                            Cow::Borrowed(buf)
                        };

                        let eof = n == 0;

                        if let Some(action) = opts.output_action.as_mut() {
                            let ctx = Context::new(
                                &mut *opts.session,
                                &mut opts.output,
                                &buf,
                                eof,
                                &mut opts.state,
                            );
                            (action)(ctx)?;
                        }

                        if eof {
                            return Ok(());
                        }

                        spin_write(&mut opts.output, &buf)?;
                        spin_flush(&mut opts.output)?;
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => return Err(err.into()),
                }

                // Set interest in the next readability event.
                poller.modify(&opts.session.get_stream().as_raw_fd(), Event::readable(1))?;
            }
        }

        if let Some(action) = opts.idle_action.as_mut() {
            let ctx = Context::new(
                &mut *opts.session,
                &mut opts.output,
                &[],
                false,
                &mut opts.state,
            );
            (action)(ctx)?;
        }
    }
}

#[cfg(all(windows, not(feature = "async"), feature = "polling"))]
fn interact_polling_on_thread<
    State,
    Output,
    Input,
    InputFilter,
    OutputFilter,
    InputAction,
    OutputAction,
    IdleAction,
>(
    session: &mut Session,
    mut output: Output,
    input: Input,
    state: &mut State,
    escape_character: u8,
    mut input_filter: Option<InputFilter>,
    mut output_filter: Option<OutputFilter>,
    mut input_action: Option<InputAction>,
    mut output_action: Option<OutputAction>,
    mut idle_action: Option<IdleAction>,
) -> Result<(), Error>
where
    Input: Read + Send + 'static,
    Output: Write,
    InputFilter: FnMut(&[u8]) -> Result<Cow<[u8]>, Error>,
    OutputFilter: FnMut(&[u8]) -> Result<Cow<[u8]>, Error>,
    InputAction: FnMut(Context<'_, &mut Session, &mut Output, &mut State>) -> Result<(), Error>,
    OutputAction: FnMut(Context<'_, &mut Session, &mut Output, &mut State>) -> Result<(), Error>,
    IdleAction: FnMut(Context<'_, &mut Session, &mut Output, &mut State>) -> Result<(), Error>,
{
    use crate::{
        error::to_io_error,
        waiter::{Recv, Wait2},
    };

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
            return Ok(());
        }

        // Wait for at least one I/O event.
        let event = poller.recv().map_err(to_io_error(""))?;
        match event {
            Recv::R1(b) => match b {
                Ok(b) => {
                    let eof = b.is_none();
                    let n = if eof { 0 } else { 1 };
                    let buf = b.map_or([0], |b| [b]);
                    let buf = &buf[..n];

                    let buf = if let Some(filter) = input_filter.as_mut() {
                        (filter)(buf)?
                    } else {
                        Cow::Borrowed(buf)
                    };

                    if let Some(action) = input_action.as_mut() {
                        let ctx = Context::new(&mut *session, &mut output, &buf, eof, &mut *state);
                        (action)(ctx)?;
                    }

                    if eof {
                        return Ok(());
                    }

                    let escape_char_pos = buf.iter().position(|c| *c == escape_character);
                    match escape_char_pos {
                        Some(pos) => {
                            session.write_all(&buf[..pos])?;
                            return Ok(());
                        }
                        None => session.write_all(&buf[..])?,
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err.into()),
            },
            Recv::R2(b) => match b {
                Ok(b) => {
                    let eof = b.is_none();
                    let n = if eof { 0 } else { 1 };
                    let buf = b.map_or([0], |b| [b]);
                    let buf = &buf[..n];

                    let buf = if let Some(filter) = output_filter.as_mut() {
                        (filter)(buf)?
                    } else {
                        Cow::Borrowed(buf)
                    };

                    if let Some(action) = output_action.as_mut() {
                        let ctx = Context::new(&mut *session, &mut output, &buf, eof, &mut *state);
                        (action)(ctx)?;
                    }

                    if eof {
                        return Ok(());
                    }

                    output.write_all(&buf)?;
                    output.flush()?;
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err.into()),
            },
            Recv::Timeout => {
                if let Some(action) = idle_action.as_mut() {
                    let ctx = Context::new(&mut *session, &mut output, &[], false, &mut *state);
                    (action)(ctx)?;
                }
            }
        }
    }
}

#[cfg(feature = "async")]
async fn interact_async<
    State,
    Stream,
    Output,
    Input,
    InputFilter,
    OutputFilter,
    InputAction,
    OutputAction,
    IdleAction,
>(
    opts: &mut InteractSession<
        '_,
        State,
        Session<Proc, Stream>,
        Output,
        Input,
        InputFilter,
        OutputFilter,
        InputAction,
        OutputAction,
        IdleAction,
    >,
) -> Result<(), Error>
where
    Stream: futures_lite::AsyncRead + futures_lite::AsyncWrite + Unpin,
    Input: futures_lite::AsyncRead + Unpin,
    Output: Write,
    InputFilter: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    OutputFilter: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    InputAction: FnMut(
        Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
    ) -> Result<(), Error>,
    OutputAction: FnMut(
        Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
    ) -> Result<(), Error>,
    IdleAction: FnMut(
        Context<'_, &mut Session<Proc, Stream>, &mut Output, &mut State>,
    ) -> Result<(), Error>,
{
    use std::io;

    use futures_lite::{AsyncReadExt, AsyncWriteExt};

    let mut stdin_buf = [0; 512];
    let mut proc_buf = [0; 512];
    loop {
        // In case where proceses exits we are trying to
        // fill buffer to run callbacks if there was something in.
        //
        // We ignore errors because there might be errors like EOCHILD etc.
        let status = opts.session.is_alive();
        if matches!(status, Ok(false)) {
            return Ok(());
        }

        #[derive(Debug)]
        enum ReadFrom {
            Stdin,
            Process,
            Timeout,
        }

        let read_process = async { (ReadFrom::Process, opts.session.read(&mut proc_buf).await) };
        let read_stdin = async { (ReadFrom::Stdin, opts.input.read(&mut stdin_buf).await) };
        let timeout = async {
            (
                ReadFrom::Timeout,
                async {
                    futures_timer::Delay::new(std::time::Duration::from_secs(5)).await;
                    io::Result::Ok(0)
                }
                .await,
            )
        };

        let read_fut = futures_lite::future::or(read_process, read_stdin);
        let (read_from, result) = futures_lite::future::or(read_fut, timeout).await;

        match read_from {
            ReadFrom::Process => {
                let n = result?;
                let buf = &proc_buf[..n];
                let buf = match opts.output_filter.as_mut() {
                    Some(filter) => (filter)(buf)?,
                    None => Cow::Borrowed(buf),
                };

                let eof = n == 0;

                if let Some(action) = &mut opts.output_action {
                    let ctx = Context::new(
                        &mut *opts.session,
                        &mut opts.output,
                        &buf,
                        eof,
                        &mut opts.state,
                    );
                    (action)(ctx)?;
                }

                if eof {
                    return Ok(());
                }

                spin_write(&mut opts.output, &buf)?;
                spin_flush(&mut opts.output)?;
            }
            ReadFrom::Stdin => {
                // We dont't print user input back to the screen.
                // In terminal mode it will be ECHOed back automatically.
                // This way we preserve terminal seetings for example when user inputs password.
                // The terminal must have been prepared before.
                match result {
                    Ok(n) => {
                        let buf = &stdin_buf[..n];
                        let buf = match opts.input_filter.as_mut() {
                            Some(filter) => (filter)(buf)?,
                            None => Cow::Borrowed(buf),
                        };

                        let eof = n == 0;

                        if let Some(action) = &mut opts.input_action {
                            let ctx = Context::new(
                                &mut *opts.session,
                                &mut opts.output,
                                &buf,
                                eof,
                                &mut opts.state,
                            );
                            (action)(ctx)?;
                        }

                        if eof {
                            return Ok(());
                        }

                        let escape_char_pos = buf.iter().position(|c| *c == opts.escape_character);
                        match escape_char_pos {
                            Some(pos) => {
                                opts.session.write_all(&buf[..pos]).await?;
                                return Ok(());
                            }
                            None => opts.session.write_all(&buf[..]).await?,
                        }
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => return Err(err.into()),
                }
            }
            ReadFrom::Timeout => {
                if let Some(action) = &mut opts.idle_action {
                    let ctx = Context::new(
                        &mut *opts.session,
                        &mut opts.output,
                        &[],
                        false,
                        &mut opts.state,
                    );
                    (action)(ctx)?;
                }

                // We need to check whether a process is alive;
                continue;
            }
        }
    }
}

fn spin_write<W>(mut writer: W, buf: &[u8]) -> std::io::Result<()>
where
    W: Write,
{
    loop {
        match writer.write_all(buf) {
            Ok(_) => return Ok(()),
            Err(err) if err.kind() != std::io::ErrorKind::WouldBlock => return Err(err),
            Err(_) => (),
        }
    }
}

fn spin_flush<W>(mut writer: W) -> std::io::Result<()>
where
    W: Write,
{
    loop {
        match writer.flush() {
            Ok(_) => return Ok(()),
            Err(err) if err.kind() != std::io::ErrorKind::WouldBlock => return Err(err),
            Err(_) => (),
        }
    }
}
