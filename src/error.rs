use std::error;
use std::fmt;
use std::fmt::Display;
use std::io;

/// An main error type used in [crate].
#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    CommandParsing,
    RegexParsing,
    ExpectTimeout,
    Eof,
    Other { message: String, origin: String },
}

impl Error {
    pub fn unknown(message: impl Into<String>, err: impl Display) -> Error {
        Self::Other {
            message: message.into(),
            origin: err.to_string(),
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
            Error::Other { message, origin } => write!(f, "An erorr {} while {} ", origin, message),
            Error::Eof => write!(f, "EOF was reached; the read may successed later"),
        }
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IO(err)
    }
}

pub fn to_io_error<E: Display>(message: &'static str) -> impl FnOnce(E) -> io::Error {
    move |e: E| io::Error::new(io::ErrorKind::Other, format!("{}; {}", message, e))
}
