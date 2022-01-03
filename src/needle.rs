use crate::error::Error;

/// Needle an interface for search of a match in a buffer.
pub trait Needle {
    /// Function returns all matches that were occured.
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error>;
}

/// Match structure represent a range of bytes where match was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    start: usize,
    end: usize,
}

impl Match {
    /// New construct's an intanse of a Match.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Start returns a start index of a match.
    pub fn start(&self) -> usize {
        self.start
    }

    /// End returns an end index of a match.
    pub fn end(&self) -> usize {
        self.end
    }
}

impl From<regex::bytes::Match<'_>> for Match {
    fn from(m: regex::bytes::Match) -> Self {
        Self::new(m.start(), m.end())
    }
}

/// Regex checks a match by regex.
pub struct Regex<Re: AsRef<str>>(pub Re);

impl<Re: AsRef<str>> Needle for Regex<Re> {
    fn check(&self, buf: &[u8], _: bool) -> Result<Vec<Match>, Error> {
        let regex = regex::bytes::Regex::new(self.0.as_ref()).map_err(|_| Error::RegexParsing)?;
        let matches = regex
            .captures_iter(buf)
            .map(|c| c.iter().flatten().map(|m| m.into()).collect::<Vec<Match>>())
            .flatten()
            .collect();
        Ok(matches)
    }
}

/// Eof consider a match when it's reached a EOF.
pub struct Eof;

impl Needle for Eof {
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        match eof {
            true => Ok(vec![Match::new(0, buf.len())]),
            false => Ok(Vec::new()),
        }
    }
}

/// NBytes matches N bytes.
pub struct NBytes(pub usize);

impl NBytes {
    fn count(&self) -> usize {
        self.0
    }
}

impl Needle for NBytes {
    fn check(&self, buf: &[u8], _: bool) -> Result<Vec<Match>, Error> {
        match buf.len() >= self.count() {
            true => Ok(vec![Match::new(0, self.count())]),
            false => Ok(Vec::new()),
        }
    }
}

impl Needle for [u8] {
    fn check(&self, buf: &[u8], _: bool) -> Result<Vec<Match>, Error> {
        if buf.len() < self.len() {
            return Ok(Vec::new());
        }

        for l_bound in 0..buf.len() {
            let r_bound = l_bound + self.len();
            if r_bound > buf.len() {
                return Ok(Vec::new());
            }

            if self == &buf[l_bound..r_bound] {
                return Ok(vec![Match::new(l_bound, r_bound)]);
            }
        }

        Ok(Vec::new())
    }
}

impl Needle for &[u8] {
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        (*self).check(buf, eof)
    }
}

impl Needle for str {
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        self.as_bytes().check(buf, eof)
    }
}

impl Needle for &str {
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        self.as_bytes().check(buf, eof)
    }
}

impl Needle for String {
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        self.as_bytes().check(buf, eof)
    }
}

impl Needle for u8 {
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        (&[*self][..]).check(buf, eof)
    }
}

impl Needle for char {
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        char::to_string(self).check(buf, eof)
    }
}

/// Any matches first Needle which returns a match.
///
/// ```no_run,ignore
/// Any(["we", "are", "here"])
/// ```
///
/// To be able to use a different type of needles you can call [Any::boxed].
///
/// ```no_run,ignore
/// Any::boxed(vec![Box::new("we"), Box::new(NBytes(3))])
/// ```
pub struct Any<I>(pub I);

impl Any<Vec<Box<dyn Needle>>> {
    pub fn boxed(v: Vec<Box<dyn Needle>>) -> Self {
        Self(v)
    }
}

impl<T> Needle for Any<&[T]>
where
    T: Needle,
{
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        for needle in self.0.iter() {
            let found = needle.check(buf, eof)?;
            if !found.is_empty() {
                return Ok(found);
            }
        }

        Ok(Vec::new())
    }
}

impl<T> Needle for Any<Vec<T>>
where
    T: Needle,
{
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        Any(self.0.as_slice()).check(buf, eof)
    }
}

impl<T, const N: usize> Needle for Any<[T; N]>
where
    T: Needle,
{
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        Any(&self.0[..]).check(buf, eof)
    }
}

impl<'a, T, const N: usize> Needle for Any<&'a [T; N]>
where
    T: Needle,
{
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        Any(&self.0[..]).check(buf, eof)
    }
}

impl<T: Needle> Needle for &T {
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        T::check(self, buf, eof)
    }
}

impl Needle for Box<dyn Needle> {
    fn check(&self, buf: &[u8], eof: bool) -> Result<Vec<Match>, Error> {
        self.as_ref().check(buf, eof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex() {
        assert_eq!(
            Regex("[0-9]+").check(b"+012345", false).unwrap(),
            vec![Match::new(1, 7)]
        );
        assert_eq!(
            Regex(r"\w+").check(b"What's Up Boys", false).unwrap(),
            vec![
                Match::new(0, 4),
                Match::new(5, 6),
                Match::new(7, 9),
                Match::new(10, 14)
            ]
        );
        assert_eq!(
            Regex(r"((?:\w|')+)")
                .check(b"What's Up Boys", false)
                .unwrap(),
            vec![
                Match::new(0, 6),
                Match::new(0, 6),
                Match::new(7, 9),
                Match::new(7, 9),
                Match::new(10, 14),
                Match::new(10, 14)
            ]
        );
        assert_eq!(
            Regex(r"(\w+)=(\w+)").check(b"asd=123", false).unwrap(),
            vec![Match::new(0, 7), Match::new(0, 3), Match::new(4, 7)]
        );
    }

    #[test]
    fn test_eof() {
        assert_eq!(Eof.check(b"qwe", true).unwrap(), vec![Match::new(0, 3)]);
        assert_eq!(Eof.check(b"qwe", false).unwrap(), vec![]);
    }

    #[test]
    fn test_n_bytes() {
        assert_eq!(
            NBytes(1).check(b"qwe", false).unwrap(),
            vec![Match::new(0, 1)]
        );
        assert_eq!(
            NBytes(0).check(b"qwe", false).unwrap(),
            vec![Match::new(0, 0)]
        );
        assert_eq!(NBytes(10).check(b"qwe", false).unwrap(), vec![]);
    }

    #[test]
    fn test_str() {
        assert_eq!(
            "wer".check(b"qwerty", false).unwrap(),
            vec![Match::new(1, 4)]
        );
        assert_eq!("123".check(b"qwerty", false).unwrap(), vec![]);
        assert_eq!("".check(b"qwerty", false).unwrap(), vec![Match::new(0, 0)]);
    }

    #[test]
    fn test_bytes() {
        assert_eq!(
            b"wer".check(b"qwerty", false).unwrap(),
            vec![Match::new(1, 4)]
        );
        assert_eq!(b"123".check(b"qwerty", false).unwrap(), vec![]);
        assert_eq!(b"".check(b"qwerty", false).unwrap(), vec![Match::new(0, 0)]);
    }

    #[test]
    fn test_bytes_ref() {
        assert_eq!(
            (&[b'q', b'w', b'e']).check(b"qwerty", false).unwrap(),
            vec![Match::new(0, 3)]
        );
        assert_eq!(
            (&[b'1', b'2', b'3']).check(b"qwerty", false).unwrap(),
            vec![]
        );
        assert_eq!(
            (&[]).check(b"qwerty", false).unwrap(),
            vec![Match::new(0, 0)]
        );
    }

    #[test]
    fn test_byte() {
        assert_eq!(
            (b'3').check(b"1234", false).unwrap(),
            vec![Match::new(2, 3)]
        );
        assert_eq!((b'3').check(b"1234", true).unwrap(), vec![Match::new(2, 3)]);
    }

    #[test]
    fn test_char() {
        for eof in [false, true] {
            assert_eq!(
                ('😘').check("😁😄😅😓😠😘😌".as_bytes(), eof).unwrap(),
                vec![Match::new(20, 24)]
            );
        }
    }

    #[test]
    fn test_any() {
        assert_eq!(
            Any(vec![Box::new("we") as Box<dyn Needle>, Box::new(NBytes(3)),])
                .check(b"qwerty", false)
                .unwrap(),
            vec![Match::new(1, 3)]
        );
        assert_eq!(
            Any::boxed(vec![Box::new("123"), Box::new(NBytes(100))])
                .check(b"qwerty", false)
                .unwrap(),
            vec![],
        );
        assert_eq!(
            Any(["123", "234", "rty"]).check(b"qwerty", false).unwrap(),
            vec![Match::new(3, 6)]
        );
        assert_eq!(
            Any(&["123", "234", "rty"][..])
                .check(b"qwerty", false)
                .unwrap(),
            vec![Match::new(3, 6)]
        );
        assert_eq!(
            Any(&["123", "234", "rty"]).check(b"qwerty", false).unwrap(),
            vec![Match::new(3, 6)]
        );
    }
}
