use crate::error::Error;

pub trait Expect {
    fn expect(&self, buf: &[u8], eof: bool) -> Result<Option<Match>, Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    start: usize,
    end: usize,
}

impl Match {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }
}

impl From<regex::Match<'_>> for Match {
    fn from(m: regex::Match) -> Self {
        Self::new(m.start(), m.end())
    }
}

pub struct Regex<Re: AsRef<str>>(pub Re);

impl<Re: AsRef<str>> Expect for Regex<Re> {
    fn expect(&self, buf: &[u8], _: bool) -> Result<Option<Match>, Error> {
        let regex = regex::Regex::new(self.0.as_ref()).map_err(|_| Error::RegexParsing)?;
        match String::from_utf8(buf.to_vec()) {
            Ok(s) => Ok(regex.find(&s).map(|m| m.into())), // indexes in UTF-8 are broken?
            _ => Ok(None),
        }
    }
}

pub struct Eof;

impl Expect for Eof {
    fn expect(&self, buf: &[u8], eof: bool) -> Result<Option<Match>, Error> {
        match eof {
            true => Ok(Some(Match::new(0, buf.len()))),
            false => Ok(None),
        }
    }
}

pub struct NBytes(pub usize);

impl NBytes {
    fn count(&self) -> usize {
        self.0
    }
}

impl Expect for NBytes {
    fn expect(&self, buf: &[u8], _: bool) -> Result<Option<Match>, Error> {
        match buf.len() > self.count() {
            true => Ok(Some(Match::new(0, self.count()))),
            false => Ok(None),
        }
    }
}

impl<S: AsRef<str>> Expect for S {
    fn expect(&self, buf: &[u8], _: bool) -> Result<Option<Match>, Error> {
        String::from_utf8(buf.to_vec())
            .map_or(None, |s| {
                let needle = self.as_ref();
                // indexes aren't corrent in UTF-8?
                s.find(needle)
                    .map(|pos| Match::new(pos, pos + needle.len()))
            })
            .map(Ok)
            .transpose()
    }
}
