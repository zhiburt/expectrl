//! This module contains a [`InteractSession`] which runs an interact session with IO.

// todo: PtyProcess wait_echo optimize by not looping when timout is none

use std::{
    borrow::Cow,
    io::{ErrorKind, Write},
};

#[cfg(not(feature = "async"))]
use std::io::Read;

#[cfg(feature = "async")]
use std::{io, time::Duration};

#[cfg(feature = "async")]
use futures_timer::Delay;

#[cfg(feature = "async")]
use futures_lite::{
    future,
    io::{AsyncRead, AsyncWrite},
    AsyncReadExt, AsyncWriteExt,
};

use crate::{
    process::{Healthcheck, Termios},
    Error,
};

#[cfg(not(feature = "async"))]
use crate::Expect;

#[cfg(feature = "async")]
use crate::AsyncExpect;

use crate::interact::Context;
#[cfg(all(not(feature = "async"), not(feature = "polling")))]
use crate::process::NonBlocking;

#[cfg(unix)]
use crate::process::unix::WaitStatus;

type ExpectResult<T> = Result<T, Error>;

/// InteractConfig represents options of an interactive session.
pub struct InteractSession<Session, Input, Output, State> {
    session: Session,
    input: Input,
    output: Output,
    escape_character: u8,
    #[cfg(unix)]
    status: Option<WaitStatus>,
    opts: InteractOptions<Session, Input, Output, State>,
}

/// Interact options (aka callbacks you can set to be callled being in an interactive mode).
struct InteractOptions<S, I, O, C> {
    state: C,
    input_filter: Option<OptFilter>,
    output_filter: Option<OptFilter>,
    input_action: Option<OptAction<S, I, O, C>>,
    output_action: Option<OptAction<S, I, O, C>>,
    idle_action: Option<OptAction<S, I, O, C>>,
}

type OptAction<S, I, O, C> = Box<dyn FnMut(Context<'_, S, I, O, C>) -> ExpectResult<bool>>;

type OptFilter = Box<dyn FnMut(&[u8]) -> ExpectResult<Cow<'_, [u8]>>>;

impl<S, I, O, C> InteractSession<S, I, O, C> {
    /// Default escape character. <Ctrl-\]>
    pub const ESCAPE: u8 = 29;

    /// Creates a new object of [`InteractSession`].
    pub fn new(session: S, input: I, output: O, state: C) -> InteractSession<S, I, O, C> {
        InteractSession {
            input,
            output,
            session,
            escape_character: Self::ESCAPE,
            opts: InteractOptions {
                state,
                input_filter: None,
                output_filter: None,
                input_action: None,
                output_action: None,
                idle_action: None,
            },
            #[cfg(unix)]
            status: None,
        }
    }

    /// Sets an escape character after seen which the interact interactions will be stopped
    /// and controll will be returned to a caller process.
    pub fn set_escape_character(mut self, c: u8) -> Self {
        self.escape_character = c;
        self
    }

    /// Returns a status of spawned session if it was exited.
    ///
    /// If [`Self::spawn`] returns false but this method returns None it means that a child process was shutdown by various reasons.
    /// Which sometimes happens and it's not considered to be a valid [`WaitStatus`], so None is returned.
    ///
    /// [`Self::spawn`]: crate::interact::InteractSession::spawn
    #[cfg(unix)]
    pub fn get_status(&self) -> Option<WaitStatus> {
        self.status
    }
}

impl<S, I, O, C> InteractSession<S, I, O, C> {
    /// Set a state
    pub fn with_state<State>(self, state: State) -> InteractSession<S, I, O, State> {
        let mut s = InteractSession::new(self.session, self.input, self.output, state);
        s.escape_character = self.escape_character;
        #[cfg(unix)]
        {
            s.status = self.status;
        }

        s
    }

    /// Get a reference on state
    pub fn get_state(&self) -> &C {
        &self.opts.state
    }

    /// Get a mut reference on state
    pub fn get_state_mut(&mut self) -> &mut C {
        &mut self.opts.state
    }

    /// Returns a inner state.
    pub fn into_state(self) -> C {
        self.opts.state
    }

    /// Sets the output filter.
    /// The output_filter will be passed all the output from the child process.
    ///
    /// The filter isn't applied to user's `read` calls through the [`Context`] in callbacks.
    pub fn set_output_filter<F>(&mut self, filter: F) -> &mut Self
    where
        F: FnMut(&[u8]) -> ExpectResult<Cow<'_, [u8]>> + 'static,
    {
        self.opts.output_filter = Some(Box::new(filter));
        self
    }

    /// Sets the input filter.
    /// The input_filter will be passed all the keyboard input from the user.
    ///
    /// The input_filter is run BEFORE the check for the escape_character.
    /// The filter is called BEFORE calling a on_input callback if it's set.
    pub fn set_input_filter<F>(&mut self, filter: F) -> &mut Self
    where
        F: FnMut(&[u8]) -> ExpectResult<Cow<'_, [u8]>> + 'static,
    {
        self.opts.input_filter = Some(Box::new(filter));
        self
    }

    /// Puts a hanlder which will be called when users input is detected.
    ///
    /// Be aware that currently async version doesn't take a Session as an argument.
    /// See <https://github.com/zhiburt/expectrl/issues/16>.
    pub fn set_input_action<F>(&mut self, action: F) -> &mut Self
    where
        F: FnMut(Context<'_, S, I, O, C>) -> ExpectResult<bool> + 'static,
    {
        self.opts.input_action = Some(Box::new(action));
        self
    }

    /// Puts a hanlder which will be called when process output is detected.
    ///
    /// IMPORTANT:
    ///
    /// Please be aware that your use of [Session::expect], [Session::check] and any `read` operation on session
    /// will cause the read bytes not to apeard in the output stream!
    pub fn set_output_action<F>(&mut self, action: F) -> &mut Self
    where
        F: FnMut(Context<'_, S, I, O, C>) -> ExpectResult<bool> + 'static,
    {
        self.opts.output_action = Some(Box::new(action));
        self
    }

    /// Puts a handler which will be called on each interaction when no input is detected.
    pub fn set_idle_action<F>(&mut self, action: F) -> &mut Self
    where
        F: FnMut(Context<'_, S, I, O, C>) -> ExpectResult<bool> + 'static,
    {
        self.opts.idle_action = Some(Box::new(action));
        self
    }
}

#[cfg(not(any(feature = "async", feature = "polling")))]
impl<S, I, O, C> InteractSession<S, I, O, C>
where
    I: Read,
    O: Write,
    S: Expect + Termios + Healthcheck<Status = WaitStatus> + NonBlocking + Write + Read,
{
    /// Runs the session.
    ///
    /// See [`Session::interact`].
    ///
    /// [`Session::interact`]: crate::session::Session::interact
    pub fn spawn(&mut self) -> ExpectResult<bool> {
        #[cfg(unix)]
        {
            let is_echo = self.session.is_echo()?;
            if !is_echo {
                let _ = self.session.set_echo(true);
            }

            self.status = None;
            let is_alive = interact_buzy_loop(self)?;

            if !is_echo {
                let _ = self.session.set_echo(false);
            }

            Ok(is_alive)
        }

        #[cfg(windows)]
        {
            interact_buzy_loop(self)
        }
    }
}

#[cfg(all(unix, feature = "polling", not(feature = "async")))]
impl<S, I, O> InteractSession<&mut Session<OsProcess, S>, I, O>
where
    I: Read + std::os::unix::io::AsRawFd,
    O: Write,
    S: Write + Read + std::os::unix::io::AsRawFd,
{
    /// Runs the session.
    ///
    /// See [`Session::interact`].
    ///
    /// [`Session::interact`]: crate::session::Session::interact
    pub fn spawn<C, IF, OF, IA, OA, WA, OPS>(&mut self, mut ops: OPS) -> Result<bool, Error>
    where
        OPS: BorrowMut<InteractOptions<C, IF, OF, IA, OA, WA>>,
        IF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
        OF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
        IA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
        OA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
        WA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
    {
        let is_echo = self
            .session
            .get_process()
            .get_echo()
            .map_err(|e| Error::unknown("failed to get echo", e.to_string()))?;
        if !is_echo {
            let _ = self.session.get_process_mut().set_echo(true, None);
        }

        self.status = None;
        let is_alive = interact_polling(self, ops.borrow_mut())?;

        if !is_echo {
            let _ = self.session.get_process_mut().set_echo(false, None);
        }

        Ok(is_alive)
    }
}

#[cfg(feature = "async")]
impl<S, I, O, C> InteractSession<S, I, O, C>
where
    I: AsyncRead + Unpin,
    O: AsyncWrite + Unpin,
    S: AsyncExpect + Termios + Healthcheck<Status = WaitStatus> + AsyncWrite + AsyncRead + Unpin,
{
    /// Runs the session.
    ///
    /// See [`Session::interact`].
    ///
    /// [`Session::interact`]: crate::session::Session::interact
    pub async fn spawn(&mut self) -> Result<bool, Error> {
        #[cfg(unix)]
        {
            let is_echo = self.session.is_echo().map_err(Error::IO)?;
            if !is_echo {
                let _ = self.session.set_echo(true);
            }

            let is_alive = interact_async(self).await?;

            if !is_echo {
                let _ = self.session.set_echo(false);
            }

            Ok(is_alive)
        }

        #[cfg(windows)]
        {
            interact_async(self, opts.borrow_mut()).await
        }
    }
}

#[cfg(all(windows, feature = "polling", not(feature = "async")))]
impl<I, O> InteractSession<&mut Session, I, O>
where
    I: Read + Clone + Send + 'static,
    O: Write,
{
    /// Runs the session.
    ///
    /// See [`Session::interact`].
    ///
    /// [`Session::interact`]: crate::session::Session::interact
    pub fn spawn<C, IF, OF, IA, OA, WA, OPS>(&mut self, mut ops: OPS) -> Result<bool, Error>
    where
        OPS: BorrowMut<InteractOptions<C, IF, OF, IA, OA, WA>>,
        IF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
        OF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
        IA: FnMut(Context<'_, Session, I, O, C>) -> Result<bool, Error>,
        OA: FnMut(Context<'_, Session, I, O, C>) -> Result<bool, Error>,
        WA: FnMut(Context<'_, Session, I, O, C>) -> Result<bool, Error>,
    {
        interact_polling_on_thread(self, ops.borrow_mut())
    }
}

impl<S, I, O, C> std::fmt::Debug for InteractSession<S, I, O, C>
where
    S: std::fmt::Debug,
    I: std::fmt::Debug,
    O: std::fmt::Debug,
    C: std::fmt::Debug,
{
    #[rustfmt::skip]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InteractSession")
            .field("session", &self.session)
            .field("input", &self.input)
            .field("output", &self.output)
            .field("escape_character", &self.escape_character)
            .field("status", &self.status)
            .field("state", &std::ptr::addr_of!(self.opts.state))
            .field("opts:on_idle", &get_pointer(&self.opts.idle_action))
            .field("opts:on_input", &get_pointer(&self.opts.input_action))
            .field("opts:on_output", &get_pointer(&self.opts.output_action))
            .field("opts:input_filter", &get_pointer(&self.opts.input_filter))
            .field("opts:output_filter", &get_pointer(&self.opts.output_filter))
            .finish()
    }
}

#[cfg(all(unix, not(feature = "async"), not(feature = "polling")))]
fn interact_buzy_loop<S, O, I, C>(s: &mut InteractSession<S, I, O, C>) -> ExpectResult<bool>
where
    S: Healthcheck<Status = WaitStatus> + NonBlocking + Write + Read,
    O: Write,
    I: Read,
{
    let mut buf = [0; 512];

    loop {
        let status = get_status(&s.session)?;
        if !matches!(status, Some(WaitStatus::StillAlive)) {
            s.status = status;
            return Ok(false);
        }

        if let Some(n) = try_read(&mut s.session, &mut buf)? {
            let eof = n == 0;
            let buf = &buf[..n];
            let buf = call_filter(s.opts.output_filter.as_mut(), buf)?;

            #[rustfmt::skip]
            let exit = opt_action(
                Context::new(&mut s.session, &mut s.input, &mut s.output, &mut s.opts.state, &buf, eof),
                &mut s.opts.output_action,
            )?;
            if eof || exit {
                return Ok(true);
            }

            spin_write(&mut s.output, &buf)?;
            spin_flush(&mut s.output)?;
        }

        // We dont't print user input back to the screen.
        // In terminal mode it will be ECHOed back automatically.
        // This way we preserve terminal seetings for example when user inputs password.
        // The terminal must have been prepared before.
        match s.input.read(&mut buf) {
            Ok(n) => {
                let eof = n == 0;
                let buf = &buf[..n];
                let buf = call_filter(s.opts.input_filter.as_mut(), buf)?;

                #[rustfmt::skip]
                let exit = opt_action(
                    Context::new(&mut s.session, &mut s.input, &mut s.output, &mut s.opts.state, &buf, eof),
                    &mut s.opts.input_action,
                )?;
                if eof | exit {
                    return Ok(true);
                }

                let escape_char_position = buf.iter().position(|c| *c == s.escape_character);
                match escape_char_position {
                    Some(pos) => {
                        s.session.write_all(&buf[..pos])?;
                        return Ok(true);
                    }
                    None => {
                        s.session.write_all(&buf[..])?;
                    }
                }
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        #[rustfmt::skip]
        let exit = opt_action(
            Context::new(&mut s.session, &mut s.input, &mut s.output, &mut s.opts.state, &buf, false),
            &mut s.opts.idle_action,
        )?;
        if exit {
            return Ok(true);
        }
    }
}

#[cfg(all(unix, not(feature = "async"), feature = "polling"))]
fn interact_polling<S, O, I, C, IF, OF, IA, OA, WA>(
    interact: &mut InteractSession<&mut Session<OsProcess, S>, I, O>,
    opts: &mut InteractOptions<C, IF, OF, IA, OA, WA>,
) -> Result<bool, Error>
where
    S: Write + Read + std::os::unix::io::AsRawFd,
    I: Read + std::os::unix::io::AsRawFd,
    O: Write,
    IF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    OF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    IA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
    OA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
    WA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
{
    use polling::{Event, Poller};

    // Create a poller and register interest in readability on the socket.
    let poller = Poller::new()?;
    poller.add(interact.input.as_raw_fd(), Event::readable(0))?;
    poller.add(
        interact.session.get_stream().as_raw_fd(),
        Event::readable(1),
    )?;

    let mut buf = [0; 512];

    // The event loop.
    let mut events = Vec::new();
    loop {
        let status = get_status(interact.session)?;
        if !matches!(status, Some(crate::WaitStatus::StillAlive)) {
            interact.status = status;
            return Ok(false);
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
                match interact.input.read(&mut buf) {
                    Ok(n) => {
                        let eof = n == 0;
                        let buf = &buf[..n];
                        let buf = call_filter(opts.input_filter.as_mut(), buf)?;

                        let exit = call_action(
                            opts.input_action.as_mut(),
                            interact.session,
                            &mut interact.input,
                            &mut interact.output,
                            &mut opts.state,
                            &buf,
                            eof,
                        )?;

                        if eof || exit {
                            return Ok(true);
                        }

                        let escape_char_pos =
                            buf.iter().position(|c| *c == interact.escape_character);
                        match escape_char_pos {
                            Some(pos) => {
                                interact.session.write_all(&buf[..pos]).map_err(Error::IO)?;
                                return Ok(true);
                            }
                            None => interact.session.write_all(&buf[..])?,
                        }
                    }
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {}
                    Err(err) => return Err(err.into()),
                }

                // Set interest in the next readability event.
                poller.modify(interact.input.as_raw_fd(), Event::readable(0))?;
            }

            if ev.key == 1 {
                match interact.session.read(&mut buf) {
                    Ok(n) => {
                        let eof = n == 0;
                        let buf = &buf[..n];
                        let buf = call_filter(opts.output_filter.as_mut(), buf)?;

                        let exit = call_action(
                            opts.output_action.as_mut(),
                            interact.session,
                            &mut interact.input,
                            &mut interact.output,
                            &mut opts.state,
                            &buf,
                            eof,
                        )?;

                        if eof || exit {
                            return Ok(true);
                        }

                        spin_write(&mut interact.output, &buf)?;
                        spin_flush(&mut interact.output)?;
                    }
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {}
                    Err(err) => return Err(err.into()),
                }

                // Set interest in the next readability event.
                poller.modify(
                    interact.session.get_stream().as_raw_fd(),
                    Event::readable(1),
                )?;
            }
        }

        let exit = call_action(
            opts.idle_action.as_mut(),
            interact.session,
            &mut interact.input,
            &mut interact.output,
            &mut opts.state,
            &[],
            false,
        )?;

        if exit {
            return Ok(true);
        }
    }
}

#[cfg(all(windows, not(feature = "async"), feature = "polling"))]
fn interact_polling_on_thread<O, I, C, IF, OF, IA, OA, WA>(
    interact: &mut InteractSession<&mut Session, I, O>,
    opts: &mut InteractOptions<C, IF, OF, IA, OA, WA>,
) -> Result<bool, Error>
where
    I: Read + Clone + Send + 'static,
    O: Write,
    IF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    OF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    IA: FnMut(Context<'_, Session, I, O, C>) -> Result<bool, Error>,
    OA: FnMut(Context<'_, Session, I, O, C>) -> Result<bool, Error>,
    WA: FnMut(Context<'_, Session, I, O, C>) -> Result<bool, Error>,
{
    use crate::{
        error::to_io_error,
        waiter::{Recv, Wait2},
    };

    // Create a poller and register interest in readability on the socket.
    let stream = interact
        .session
        .get_stream()
        .try_clone()
        .map_err(to_io_error(""))?;
    let mut poller = Wait2::new(interact.input.clone(), stream);

    loop {
        // In case where proceses exits we are trying to
        // fill buffer to run callbacks if there was something in.
        //
        // We ignore errors because there might be errors like EOCHILD etc.
        if interact.session.is_alive()? {
            return Ok(false);
        }

        // Wait for at least one I/O event.
        let event = poller.recv().map_err(to_io_error(""))?;
        match event {
            Recv::R1(b) => match b {
                Ok(b) => {
                    let buf = b.map_or([0], |b| [b]);
                    let eof = b.is_none();
                    let n = if eof { 0 } else { 1 };
                    let buf = &buf[..n];

                    let buf = call_filter(opts.input_filter.as_mut(), buf)?;

                    let exit = call_action(
                        opts.input_action.as_mut(),
                        interact.session,
                        &mut interact.input,
                        &mut interact.output,
                        &mut opts.state,
                        &buf,
                        eof,
                    )?;

                    if eof || exit {
                        return Ok(true);
                    }

                    // todo: replace all of these by 1 by 1 write
                    let escape_char_pos = buf.iter().position(|c| *c == interact.escape_character);
                    match escape_char_pos {
                        Some(pos) => {
                            interact.session.write_all(&buf[..pos])?;
                            return Ok(true);
                        }
                        None => interact.session.write_all(&buf[..])?,
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err.into()),
            },
            Recv::R2(b) => match b {
                Ok(b) => {
                    let buf = b.map_or([0], |b| [b]);
                    let eof = b.is_none();
                    let n = if eof { 0 } else { 1 };
                    let buf = &buf[..n];

                    let buf = call_filter(opts.output_filter.as_mut(), buf)?;

                    let exit = call_action(
                        opts.output_action.as_mut(),
                        interact.session,
                        &mut interact.input,
                        &mut interact.output,
                        &mut opts.state,
                        &buf,
                        eof,
                    )?;

                    if eof || exit {
                        return Ok(true);
                    }

                    interact.output.write_all(&buf)?;
                    interact.output.flush()?;
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err.into()),
            },
            Recv::Timeout => {
                let exit = call_action(
                    opts.idle_action.as_mut(),
                    interact.session,
                    &mut interact.input,
                    &mut interact.output,
                    &mut opts.state,
                    &[],
                    false,
                )?;

                if exit {
                    return Ok(true);
                }
            }
        }
    }
}

#[cfg(all(unix, feature = "async"))]
async fn interact_async<S, O, I, C>(s: &mut InteractSession<S, I, O, C>) -> Result<bool, Error>
where
    S: Healthcheck<Status = WaitStatus> + AsyncRead + AsyncWrite + Unpin,
    I: AsyncRead + Unpin,
    O: AsyncWrite + Unpin,
{
    #[derive(Debug)]
    enum ReadFrom {
        Input,
        Proc,
        Timeout,
    }

    const TIMEOUT: Duration = Duration::from_secs(5);
    let mut input_buf = [0; 512];
    let mut proc_buf = [0; 512];

    loop {
        let status = get_status(&s.session)?;
        if !matches!(status, Some(WaitStatus::StillAlive)) {
            s.status = status;
            return Ok(false);
        }

        let read_process = async { (ReadFrom::Proc, s.session.read(&mut proc_buf).await) };
        let read_input = async { (ReadFrom::Input, s.input.read(&mut input_buf).await) };
        let timeout = async { (ReadFrom::Timeout, async_timeout(TIMEOUT).await) };

        let read_any = future::or(read_process, read_input);
        let read_output = future::or(read_any, timeout).await;
        let read_target = read_output.0;
        let read_result = read_output.1;

        match read_target {
            ReadFrom::Proc => {
                let n = read_result?;
                let eof = n == 0;
                let buf = &proc_buf[..n];
                let buf = call_filter(s.opts.output_filter.as_mut(), buf)?;

                let exit = run_action_output(s, &buf, eof)?;

                if eof || exit {
                    return Ok(true);
                }

                s.output.write(&buf).await?;
                s.output.flush().await?;
            }
            ReadFrom::Input => {
                // We dont't print user input back to the screen.
                // In terminal mode it will be ECHOed back automatically.
                // This way we preserve terminal seetings for example when user inputs password.
                // The terminal must have been prepared before.
                match read_result {
                    Ok(n) => {
                        let eof = n == 0;
                        let buf = &input_buf[..n];
                        let buf = call_filter(s.opts.output_filter.as_mut(), buf)?;

                        let exit = run_action_input(s, &buf, eof)?;

                        if eof || exit {
                            return Ok(true);
                        }

                        let escape_char_pos = buf.iter().position(|c| *c == s.escape_character);
                        match escape_char_pos {
                            Some(pos) => {
                                s.session.write_all(&buf[..pos]).await?;
                                return Ok(true);
                            }
                            None => s.session.write_all(&buf[..]).await?,
                        }
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => return Err(err.into()),
                }
            }
            ReadFrom::Timeout => {
                let exit = run_action_idle(s, &[], false)?;
                if exit {
                    return Ok(true);
                }
            }
        }
    }
}

#[cfg(feature = "async")]
async fn async_timeout(timeout: Duration) -> io::Result<usize> {
    Delay::new(timeout).await;
    io::Result::Ok(0)
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

#[rustfmt::skip]
fn run_action_input<S, I, O, C>(s: &mut InteractSession<S, I, O, C>, buf: &[u8], eof: bool) -> ExpectResult<bool> {
    let ctx = Context::new(&mut s.session, &mut s.input, &mut s.output, &mut s.opts.state, &buf, eof);
    opt_action(ctx, &mut s.opts.input_action)
}

#[rustfmt::skip]
fn run_action_output<S, I, O, C>(s: &mut InteractSession<S, I, O, C>, buf: &[u8], eof: bool) -> ExpectResult<bool> {
    let ctx = Context::new(&mut s.session, &mut s.input, &mut s.output, &mut s.opts.state, &buf, eof);
    opt_action(ctx, &mut s.opts.output_action)
}

#[rustfmt::skip]
fn run_action_idle<S, I, O, C>(s: &mut InteractSession<S, I, O, C>, buf: &[u8], eof: bool) -> ExpectResult<bool> {
    let ctx = Context::new(&mut s.session, &mut s.input, &mut s.output, &mut s.opts.state, &buf, eof);
    opt_action(ctx, &mut s.opts.idle_action)
}

fn opt_action<S, I, O, C>(
    ctx: Context<'_, S, I, O, C>,
    opt: &mut Option<OptAction<S, I, O, C>>,
) -> ExpectResult<bool> {
    match opt {
        Some(action) => (action)(ctx),
        None => Ok(false),
    }
}

fn call_filter<F>(filter: Option<F>, buf: &[u8]) -> Result<Cow<'_, [u8]>, Error>
where
    F: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
{
    match filter {
        Some(mut action) => (action)(buf),
        None => Ok(Cow::Borrowed(buf)),
    }
}

#[cfg(unix)]
fn get_status<S>(session: &S) -> Result<Option<S::Status>, Error>
where
    S: Healthcheck,
{
    match session.get_status() {
        Ok(status) => Ok(Some(status)),
        Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(None),
        Err(err) => Err(Error::IO(err)),
    }
}

#[cfg(unix)]
#[cfg(not(feature = "async"))]
fn try_read<S>(session: &mut S, buf: &mut [u8]) -> ExpectResult<Option<usize>>
where
    S: NonBlocking + Read,
{
    session.set_blocking(false)?;

    let result = session.read(buf);

    session.set_blocking(true)?;

    match result {
        Ok(n) => Ok(Some(n)),
        Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(None),
        Err(err) => Err(Error::IO(err)),
    }
}

fn get_pointer<T>(ptr: &Option<Box<T>>) -> usize
where
    T: ?Sized,
{
    ptr.as_ref().map_or(0, |f| std::ptr::addr_of!(f) as usize)
}
