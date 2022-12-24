use std::error;
use std::fmt;
use std::fmt::Display;
use std::io;

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
        /// It's a custom error message
        message: String,
    },
}

impl Error {
    #[allow(dead_code)]
    pub(crate) fn unknown(message: impl Display, err: impl Display) -> Error {
        Self::Other {
            message: format!("{}: {}", message, err),
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
            Error::Other { message } => write!(f, "An other error; {} ", message),
        }
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IO(err)
    }
}

pub(crate) fn to_io_error<E: Display>(message: &'static str) -> impl FnOnce(E) -> io::Error {
    move |e: E| io::Error::new(io::ErrorKind::Other, format!("{}; {}", message, e))
}
