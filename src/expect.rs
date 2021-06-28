use std::ptr::NonNull;

use crate::error::Error;

pub trait Expect {
    type Output;

    fn expect(&self, buf: &[u8], eof: bool) -> Option<(Self::Output, Match)>;

    fn expect_str(&self, buf: &str, eof: bool) -> Option<(Self::Output, Match)> {
        self.expect(buf.as_bytes(), eof)
    }

    fn expect_length(&self, buf: &str, eof: bool) -> Option<usize> {
        None
    }
}

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

pub struct Regex {
    re: regex::Regex,
}

impl Regex {
    pub fn parse<S: AsRef<str>>(re: S) -> Result<Self, Error> {
        let regex = regex::Regex::new(re.as_ref()).map_err(|_| Error::RegexParsing)?;

        Ok(Self { re: regex })
    }
}

impl Expect for Regex {
    type Output = (String, String);

    fn expect(&self, buf: &[u8], eof: bool) -> Option<(Self::Output, Match)> {
        if buf.is_ascii() {
            let buf = String::from_utf8_lossy(buf);
            return self.expect_str(&buf, eof);
        }

        let buf = String::from_utf8_lossy(buf);
        match self.expect_str(&buf, eof) {
            Some((output, Match { start, end })) => {
                // Convert Match indexes to byte indexes
                todo!();
            }
            None => None,
        }
    }

    fn expect_str(&self, buf: &str, eof: bool) -> Option<(Self::Output, Match)> {
        self.re.find(buf).map(|m| {
            (
                (
                    buf[..m.start()].to_owned(),
                    buf[m.start()..m.start() + m.end()].to_owned(),
                ),
                m.into(),
            )
        })
    }
}

pub struct EOF;

impl Expect for EOF {
    type Output = Vec<u8>;

    fn expect(&self, buf: &[u8], eof: bool) -> Option<(Self::Output, Match)> {
        match eof {
            true => Some((buf.to_vec(), Match::new(0, buf.len()))),
            false => None,
        }
    }
}

pub struct Bytes {
    count: usize,
}

impl Bytes {
    pub fn new(count: usize) -> Self {
        Self { count }
    }
}

impl Expect for Bytes {
    type Output = Vec<u8>;

    fn expect(&self, buf: &[u8], _: bool) -> Option<(Self::Output, Match)> {
        match buf.len() < self.count {
            true => Some((buf.to_vec(), Match::new(0, self.count))),
            false => None,
        }
    }
}

impl<S: AsRef<str>> Expect for S {
    type Output = ();

    fn expect(&self, buf: &[u8], eof: bool) -> Option<(Self::Output, Match)> {
        if buf.is_ascii() {
            let buf = String::from_utf8_lossy(buf);
            return self.expect_str(&buf, eof);
        }

        let buf = String::from_utf8_lossy(buf);
        match self.expect_str(&buf, eof) {
            Some(((), Match { start, end })) => {
                // Convert Match indexes to byte indexes
                todo!();
            }
            None => None,
        }
    }

    fn expect_str(&self, buf: &str, eof: bool) -> Option<(Self::Output, Match)> {
        let needle = self.as_ref();
        buf.find(needle)
            .map(|pos| ((), Match::new(pos, pos + needle.len())))
    }
}
