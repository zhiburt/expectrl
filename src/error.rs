use std::error;
use std::fmt;
use std::fmt::Display;
use std::io;

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    Nix(nix::Error),
    CommandParsing,
    Other(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::IO(err) => write!(f, "IO error {}", err),
            Error::Nix(err) => write!(f, "Nix error {}", err),
            Error::CommandParsing => write!(f, "Can't parse a command string, please check it out"),
            Error::Other(message) => write!(f, "Error {}", message),
        }
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IO(err)
    }
}

impl From<nix::Error> for Error {
    fn from(err: nix::Error) -> Self {
        Self::Nix(err)
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}
