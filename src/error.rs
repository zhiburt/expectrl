use std::error;
use std::fmt;
use std::fmt::Display;
use std::io;

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    Nix(ptyprocess::Error),
    CommandParsing,
    RegexParsing,
    ExpectTimeout,
    Eof,
    Other(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::IO(err) => write!(f, "IO error {}", err),
            Error::Nix(err) => write!(f, "Nix error {}", err),
            Error::CommandParsing => write!(f, "Can't parse a command string, please check it out"),
            Error::RegexParsing => write!(f, "Can't parse a regex expression"),
            Error::ExpectTimeout => write!(f, "Reached a timeout for expect type of command"),
            Error::Other(message) => write!(f, "Error {}", message),
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

impl From<ptyprocess::Error> for Error {
    fn from(err: ptyprocess::Error) -> Self {
        Self::Nix(err)
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}
