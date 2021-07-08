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

impl From<regex::bytes::Match<'_>> for Match {
    fn from(m: regex::bytes::Match) -> Self {
        Self::new(m.start(), m.end())
    }
}

pub struct Regex<Re: AsRef<str>>(pub Re);

impl<Re: AsRef<str>> Expect for Regex<Re> {
    fn expect(&self, buf: &[u8], _: bool) -> Result<Option<Match>, Error> {
        let regex = regex::bytes::Regex::new(self.0.as_ref()).map_err(|_| Error::RegexParsing)?;
        Ok(regex.find(&buf).map(|m| m.into()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex() {
        assert_eq!(
            Regex("[0-9]+").expect(b"+012345", false).unwrap(),
            Some(Match::new(1, 7))
        );
        // we use a first left match
        assert_eq!(
            Regex(r"\w+").expect(b"What's Up Boys", false).unwrap(),
            Some(Match::new(0, 4))
        );
    }

    #[test]
    fn test_eof() {
        assert_eq!(Eof.expect(b"qwe", true).unwrap(), Some(Match::new(0, 3)));
        assert_eq!(Eof.expect(b"qwe", false).unwrap(), None);
    }

    #[test]
    fn test_n_bytes() {
        assert_eq!(
            NBytes(1).expect(b"qwe", false).unwrap(),
            Some(Match::new(0, 1))
        );
        assert_eq!(
            NBytes(0).expect(b"qwe", false).unwrap(),
            Some(Match::new(0, 0))
        );
        assert_eq!(NBytes(10).expect(b"qwe", false).unwrap(), None);
    }

    #[test]
    fn test_str() {
        assert_eq!(
            "wer".expect(b"qwerty", false).unwrap(),
            Some(Match::new(1, 4))
        );
        assert_eq!("123".expect(b"qwerty", false).unwrap(), None);
        assert_eq!("".expect(b"qwerty", false).unwrap(), Some(Match::new(0, 0)));
    }
}
