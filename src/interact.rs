use crate::{ControlCode, Error, Found, Needle, Session};
use std::{
    collections::HashMap,
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
    handlers: HashMap<Action, ActionFn>,
}

#[cfg(not(feature = "async"))]
type ActionFn = Box<dyn FnMut(&mut Session) -> Result<(), Error>>;

#[cfg(feature = "async")]
type ActionFn = Box<dyn FnMut(&mut Session) -> futures_lite::future::Boxed<Result<(), Error>>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Action {
    Input(String),
}

impl Default for InteractOptions {
    fn default() -> Self {
        Self {
            escape_character: ControlCode::GroupSeparator.into(), // Ctrl-]
            handlers: HashMap::new(),
        }
    }
}

impl InteractOptions {
    pub fn escape_character(mut self, c: u8) -> Self {
        self.escape_character = c;
        self
    }

    #[cfg(not(feature = "async"))]
    pub fn on_input<F>(mut self, input: impl Into<String>, f: F) -> Self
    where
        F: FnMut(&mut Session) -> Result<(), Error> + 'static,
    {
        self.handlers
            .insert(Action::Input(input.into()), Box::new(f));
        self
    }

    /// We can't use a &mut Session argument because of lifetimes.
    /// We could provide a [std::sync::Arc]<[std::sync::Mutex]<&mut [Session]>> parameter but [std::sync::Mutex] isn't recomeneded to use with async.
    /// We could use some library for Mutex like `parking_lot` but in such case the client would be piled down to the chose.
    /// So there was decided to not provide any arguments to closure.
    /// You can bypass this by wrapping [Session] on your own and use it in closure scope.
    #[cfg(feature = "async")]
    pub fn on_input<F, FF>(mut self, input: impl Into<String>, f: F) -> Self
    where
        F: FnMut() -> FF + Clone + Send + 'static,
        FF: std::future::Future<Output = Result<(), Error>> + Send,
    {
        use futures_lite::FutureExt;

        self.handlers.insert(
            Action::Input(input.into()),
            Box::new(move |_| {
                let mut f = f.clone();
                async move { f().await }.boxed()
            }),
        );
        self
    }

    #[cfg(all(unix, not(feature = "async")))]
    pub fn interact(self, session: &mut Session) -> Result<WaitStatus, Error> {
        interact(session, self)
    }

    #[cfg(all(unix, feature = "async"))]
    pub async fn interact(self, session: &mut Session) -> Result<WaitStatus, Error> {
        interact(session, self).await
    }

    #[cfg(windows)]
    pub fn interact(self, session: &mut Session) -> Result<(), Error> {
        interact(session, self)
    }

    #[cfg(not(feature = "async"))]
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

    #[cfg(feature = "async")]
    async fn check_input(
        &mut self,
        session: &mut Session,
        bytes: &mut Vec<u8>,
    ) -> Result<(), Error> {
        for (action, callback) in self.handlers.iter_mut() {
            let Action::Input(pattern) = action;

            // reuse Needle code
            let m = Needle::check(&pattern, bytes, false)?;
            if !m.is_empty() {
                let last_index_which_involved = Found::right_most_index(&m);
                bytes.drain(..last_index_which_involved);

                (callback)(session).await?;

                return Ok(());
            }
        }

        Ok(())
    }
}

#[cfg(all(unix, not(feature = "async")))]
fn interact(session: &mut Session, options: InteractOptions) -> Result<WaitStatus, Error> {
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

    let result = _interact(session, stdin, options);

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
    mut options: InteractOptions,
) -> Result<WaitStatus, Error> {
    let mut stdin_stream = Stream::new(stdin);

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
                io::stdout().write_all(&buf[..n])?;
                io::stdout().flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        match stdin_stream.try_read(&mut buf) {
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
async fn interact(session: &mut Session, options: InteractOptions) -> Result<WaitStatus, Error> {
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

    let result = _interact(session, stdin, options).await;

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
    mut options: InteractOptions,
) -> Result<WaitStatus, Error> {
    let mut stdin_stream = Stream::new(stdin);

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
                io::stdout().write_all(&buf[..n])?;
                io::stdout().flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }

        match stdin_stream.try_read(&mut buf).await {
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
                    options
                        .check_input(session, buffer_for_check.as_mut().unwrap())
                        .await?;
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err.into()),
        }
    }
}

#[cfg(windows)]
fn interact(session: &mut Session, options: InteractOptions) -> Result<(), Error> {
    // flush buffers
    self.flush()?;

    let console = conpty::console::Console::current().unwrap();
    console.set_raw().unwrap();

    let r = self._interact(&console, options);

    console.reset().unwrap();

    r
}

#[cfg(windows)]
fn _interact(
    &mut self,
    console: &conpty::console::Console,
    options: InteractOptions,
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

            // first check callbacks
            options.check_input(session, &buf[..n])?;

            let escape_char_position = buf[..n].iter().position(|c| *c == options.escape_character);
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
