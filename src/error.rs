use std::error;
use std::fmt;
use std::fmt::Display;
use std::io;

#[allow(variant_size_differences)]
/// An main error type used in [crate].
#[derive(Debug)]
pub enum Error {
    /// An Error in IO operation.
    IO(io::Error),
    /// An Error in command line parsing.
    CommandParsing,
    /// An Error in regex parsing.
    RegexParsing,
    /// An timeout was reached while waiting in expect call.
    ExpectTimeout,
    /// Unhandled EOF error.
    Eof,
    /// It maybe OS specific error or a general erorr.
    Other {
        /// The reason of the erorr.
        message: String,
        /// An underlying error message.
        err: String,
    },
}

impl Error {
    #[cfg(unix)]
    pub(crate) fn unknown(message: impl Into<String>, err: impl Into<String>) -> Error {
        Self::Other {
            message: message.into(),
            err: err.into(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::IO(err) => write!(f, "IO error {}", err),
            Error::CommandParsing => write!(f, "Can't parse a command string, please check it out"),
            Error::RegexParsing => write!(f, "Can't parse a regex expression"),
            Error::ExpectTimeout => write!(f, "Reached a timeout for expect type of command"),
            Error::Eof => write!(f, "EOF was reached; the read may successed later"),
            Error::Other { message, err } => write!(f, "Unexpected error; {}; {}", message, err),
        }
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IO(err)
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> Self {
        io::Error::other(err.to_string())
    }
}

pub(crate) fn to_io_error<E: Display>(message: &'static str) -> impl FnOnce(E) -> io::Error {
    move |e: E| io::Error::other(format!("{}; {}", message, e))
}
