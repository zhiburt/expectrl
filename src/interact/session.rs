//! This module contains a [`InteractSession`] which runs an interact session with IO.

use std::{
    borrow::{BorrowMut, Cow},
    io::{ErrorKind, Write},
};

use crate::{session::OsProcess, Error, Session};

#[cfg(not(feature = "async"))]
use std::io::Read;

use super::{Context, InteractOptions};
#[cfg(all(not(feature = "async"), not(feature = "polling")))]
use crate::process::NonBlocking;

/// InteractConfig represents options of an interactive session.
#[derive(Debug)]
pub struct InteractSession<Session, Input, Output> {
    session: Session,
    input: Input,
    output: Output,
    escape_character: u8,
    #[cfg(unix)]
    status: Option<crate::WaitStatus>,
}

impl<S, I, O> InteractSession<S, I, O> {
    /// Default escape character.
    pub const ESCAPE: u8 = 29; // Ctrl-]

    /// Creates a new object of [InteractSession].
    pub fn new(session: S, input: I, output: O) -> InteractSession<S, I, O> {
        InteractSession {
            input,
            output,
            session,
            escape_character: Self::ESCAPE,
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
    /// [`WaitStatus`]: crate::WaitStatus
    #[cfg(unix)]
    pub fn get_status(&self) -> Option<crate::WaitStatus> {
        self.status
    }
}

#[cfg(not(any(feature = "async", feature = "polling")))]
impl<S, I, O> InteractSession<&mut Session<OsProcess, S>, I, O>
where
    I: Read,
    O: Write,
    S: NonBlocking + Write + Read,
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
        #[cfg(unix)]
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
            let is_alive = interact_buzy_loop(self, ops.borrow_mut())?;

            if !is_echo {
                let _ = self.session.get_process_mut().set_echo(false, None);
            }

            Ok(is_alive)
        }

        #[cfg(windows)]
        {
            interact_buzy_loop(self, ops.borrow_mut())
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
impl<S, I, O> InteractSession<&mut Session<OsProcess, S>, I, O>
where
    I: futures_lite::AsyncRead + Unpin,
    O: Write,
    S: futures_lite::AsyncRead + futures_lite::AsyncWrite + Unpin,
{
    /// Runs the session.
    ///
    /// See [`Session::interact`].
    ///
    /// [`Session::interact`]: crate::session::Session::interact
    pub async fn spawn<C, IF, OF, IA, OA, WA, OPS>(&mut self, mut opts: OPS) -> Result<bool, Error>
    where
        OPS: BorrowMut<InteractOptions<C, IF, OF, IA, OA, WA>>,
        IF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
        OF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
        IA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
        OA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
        WA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
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

            let is_alive = interact_async(self, opts.borrow_mut()).await?;

            if !is_echo {
                let _ = self.session.set_echo(false, None);
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

#[cfg(all(not(feature = "async"), not(feature = "polling")))]
fn interact_buzy_loop<S, O, I, C, IF, OF, IA, OA, WA>(
    interact: &mut InteractSession<&mut Session<OsProcess, S>, I, O>,
    opts: &mut InteractOptions<C, IF, OF, IA, OA, WA>,
) -> Result<bool, Error>
where
    S: NonBlocking + Write + Read,
    I: Read,
    O: Write,
    IF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    OF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    IA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
    OA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
    WA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
{
    let mut buf = [0; 512];
    loop {
        #[cfg(unix)]
        {
            let status = get_status(interact.session)?;
            if !matches!(status, Some(crate::WaitStatus::StillAlive)) {
                interact.status = status;
                return Ok(false);
            }
        }

        #[cfg(windows)]
        {
            if !interact.session.is_alive()? {
                return Ok(false);
            }
        }

        match interact.session.try_read(&mut buf) {
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

                if eof | exit {
                    return Ok(true);
                }

                let escape_char_position = buf.iter().position(|c| *c == interact.escape_character);
                match escape_char_position {
                    Some(pos) => {
                        interact.session.write_all(&buf[..pos])?;
                        return Ok(true);
                    }
                    None => {
                        interact.session.write_all(&buf[..])?;
                    }
                }
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
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

#[cfg(feature = "async")]
async fn interact_async<S, O, I, C, IF, OF, IA, OA, WA>(
    interact: &mut InteractSession<&mut Session<OsProcess, S>, I, O>,
    opts: &mut InteractOptions<C, IF, OF, IA, OA, WA>,
) -> Result<bool, Error>
where
    S: futures_lite::AsyncRead + futures_lite::AsyncWrite + Unpin,
    I: futures_lite::AsyncRead + Unpin,
    O: Write,
    IF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    OF: FnMut(&[u8]) -> Result<Cow<'_, [u8]>, Error>,
    IA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
    OA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
    WA: FnMut(Context<'_, Session<OsProcess, S>, I, O, C>) -> Result<bool, Error>,
{
    use std::io;

    use futures_lite::{AsyncReadExt, AsyncWriteExt};

    let mut stdin_buf = [0; 512];
    let mut proc_buf = [0; 512];
    loop {
        #[cfg(unix)]
        {
            let status = get_status(interact.session)?;
            if !matches!(status, Some(crate::WaitStatus::StillAlive)) {
                interact.status = status;
                return Ok(false);
            }
        }

        #[cfg(windows)]
        {
            if !interact.session.is_alive()? {
                return Ok(false);
            }
        }

        #[derive(Debug)]
        enum ReadFrom {
            Stdin,
            OsProcessess,
            Timeout,
        }

        let read_process = async {
            (
                ReadFrom::OsProcessess,
                interact.session.read(&mut proc_buf).await,
            )
        };
        let read_stdin = async { (ReadFrom::Stdin, interact.input.read(&mut stdin_buf).await) };
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
            ReadFrom::OsProcessess => {
                let n = result?;
                let eof = n == 0;
                let buf = &proc_buf[..n];
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
            ReadFrom::Stdin => {
                // We dont't print user input back to the screen.
                // In terminal mode it will be ECHOed back automatically.
                // This way we preserve terminal seetings for example when user inputs password.
                // The terminal must have been prepared before.
                match result {
                    Ok(n) => {
                        let eof = n == 0;
                        let buf = &stdin_buf[..n];
                        let buf = call_filter(opts.output_filter.as_mut(), buf)?;

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
                                interact.session.write_all(&buf[..pos]).await?;
                                return Ok(true);
                            }
                            None => interact.session.write_all(&buf[..]).await?,
                        }
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => return Err(err.into()),
                }
            }
            ReadFrom::Timeout => {
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

fn call_action<F, S, I, O, C>(
    action: Option<F>,
    s: &mut S,
    r: &mut I,
    w: &mut O,
    state: &mut C,
    buf: &[u8],
    eof: bool,
) -> Result<bool, Error>
where
    F: FnMut(Context<'_, S, I, O, C>) -> Result<bool, Error>,
{
    match action {
        Some(mut action) => (action)(Context::new(s, r, w, state, buf, eof)),
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
fn get_status<S>(session: &Session<OsProcess, S>) -> Result<Option<crate::WaitStatus>, Error> {
    match session.get_process().status() {
        Ok(status) => Ok(Some(status)),
        Err(ptyprocess::errno::Errno::ECHILD | ptyprocess::errno::Errno::ESRCH) => Ok(None),
        Err(err) => Err(Error::IO(std::io::Error::new(ErrorKind::Other, err))),
    }
}
