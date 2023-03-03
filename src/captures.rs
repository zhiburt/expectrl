use std::ops::Index;

use crate::needle::Match;

/// Captures is a represention of matched pattern.
///
/// It might represent an empty match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captures {
    buf: Vec<u8>,
    matches: Vec<Match>,
}

impl Captures {
    /// New returns an instance of Found.
    pub(crate) fn new(buf: Vec<u8>, matches: Vec<Match>) -> Self {
        Self { buf, matches }
    }

    /// is_empty verifies if any matches were actually found.
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// get returns a match by index.
    pub fn get(&self, index: usize) -> Option<&[u8]> {
        self.matches
            .get(index)
            .map(|m| &self.buf[m.start()..m.end()])
    }

    /// Matches returns a list of matches.
    pub fn matches(&self) -> MatchIter<'_> {
        MatchIter::new(self)
    }

    /// before returns a bytes before match.
    pub fn before(&self) -> &[u8] {
        &self.buf[..self.left_most_index()]
    }

    /// as_bytes returns all bytes involved in a match, e.g. before the match and
    /// in a match itself.
    ///
    /// In most cases the returned value equeals to concatanted [Self::before] and [Self::matches].
    /// But sometimes like in case of [crate::Regex] it may have a grouping so [Self::matches] might overlap, therefore
    /// it will not longer be true.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    fn left_most_index(&self) -> usize {
        self.matches
            .iter()
            .map(|m| m.start())
            .min()
            .unwrap_or_default()
    }

    pub(crate) fn right_most_index(matches: &[Match]) -> usize {
        matches.iter().map(|m| m.end()).max().unwrap_or_default()
    }
}

impl Index<usize> for Captures {
    type Output = [u8];

    fn index(&self, index: usize) -> &Self::Output {
        let m = &self.matches[index];
        &self.buf[m.start()..m.end()]
    }
}

impl<'a> IntoIterator for &'a Captures {
    type Item = &'a [u8];
    type IntoIter = MatchIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        MatchIter::new(self)
    }
}

#[derive(Debug)]
pub struct MatchIter<'a> {
    buf: &'a [u8],
    matches: std::slice::Iter<'a, Match>,
}

impl<'a> MatchIter<'a> {
    fn new(captures: &'a Captures) -> Self {
        Self {
            buf: &captures.buf,
            matches: captures.matches.iter(),
        }
    }
}

impl<'a> Iterator for MatchIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        self.matches.next().map(|m| &self.buf[m.start()..m.end()])
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.matches.size_hint()
    }
}

impl ExactSizeIterator for MatchIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_captures_get() {
        let m = Captures::new(
            b"You can use iterator".to_vec(),
            vec![Match::new(0, 3), Match::new(4, 7)],
        );

        assert_eq!(m.get(0), Some(b"You".as_ref()));
        assert_eq!(m.get(1), Some(b"can".as_ref()));
        assert_eq!(m.get(2), None);

        let m = Captures::new(b"You can use iterator".to_vec(), vec![]);
        assert_eq!(m.get(0), None);
        assert_eq!(m.get(1), None);
        assert_eq!(m.get(2), None);

        let m = Captures::new(vec![], vec![]);
        assert_eq!(m.get(0), None);
        assert_eq!(m.get(1), None);
        assert_eq!(m.get(2), None);
    }

    #[test]
    #[should_panic]
    fn test_captures_get_panics_on_invalid_match() {
        let m = Captures::new(b"Hello World".to_vec(), vec![Match::new(0, 100)]);
        let _ = m.get(0);
    }

    #[test]
    fn test_captures_index() {
        let m = Captures::new(
            b"You can use iterator".to_vec(),
            vec![Match::new(0, 3), Match::new(4, 7)],
        );

        assert_eq!(&m[0], b"You".as_ref());
        assert_eq!(&m[1], b"can".as_ref());
    }

    #[test]
    #[should_panic]
    fn test_captures_index_panics_on_invalid_match() {
        let m = Captures::new(b"Hello World".to_vec(), vec![Match::new(0, 100)]);
        let _ = &m[0];
    }

    #[test]
    #[should_panic]
    fn test_captures_index_panics_on_invalid_index() {
        let m = Captures::new(b"Hello World".to_vec(), vec![Match::new(0, 3)]);
        let _ = &m[10];
    }

    #[test]
    #[should_panic]
    fn test_captures_index_panics_on_empty_match() {
        let m = Captures::new(b"Hello World".to_vec(), vec![]);
        let _ = &m[0];
    }

    #[test]
    #[should_panic]
    fn test_captures_index_panics_on_empty_captures() {
        let m = Captures::new(Vec::new(), Vec::new());
        let _ = &m[0];
    }

    #[test]
    fn test_before() {
        let m = Captures::new(b"You can use iterator".to_vec(), vec![Match::new(4, 7)]);
        assert_eq!(m.before(), b"You ".as_ref());

        let m = Captures::new(
            b"You can use iterator".to_vec(),
            vec![Match::new(0, 3), Match::new(4, 7)],
        );
        assert_eq!(m.before(), b"".as_ref());

        let m = Captures::new(b"You can use iterator".to_vec(), vec![]);
        assert_eq!(m.before(), b"".as_ref());

        let m = Captures::new(vec![], vec![]);
        assert_eq!(m.before(), b"".as_ref());
    }

    #[test]
    fn test_matches() {
        let m = Captures::new(b"You can use iterator".to_vec(), vec![Match::new(4, 7)]);
        assert_eq!(m.before(), b"You ".as_ref());

        let m = Captures::new(
            b"You can use iterator".to_vec(),
            vec![Match::new(0, 3), Match::new(4, 7)],
        );
        assert_eq!(m.before(), b"".as_ref());

        let m = Captures::new(b"You can use iterator".to_vec(), vec![]);
        assert_eq!(m.before(), b"".as_ref());

        let m = Captures::new(vec![], vec![]);
        assert_eq!(m.before(), b"".as_ref());
    }

    #[test]
    fn test_captures_into_iter() {
        assert_eq!(
            Captures::new(
                b"You can use iterator".to_vec(),
                vec![Match::new(0, 3), Match::new(4, 7)]
            )
            .into_iter()
            .collect::<Vec<&[u8]>>(),
            vec![b"You", b"can"]
        );

        assert_eq!(
            Captures::new(b"You can use iterator".to_vec(), vec![Match::new(4, 20)])
                .into_iter()
                .collect::<Vec<&[u8]>>(),
            vec![b"can use iterator"]
        );

        assert_eq!(
            Captures::new(b"You can use iterator".to_vec(), vec![])
                .into_iter()
                .collect::<Vec<&[u8]>>(),
            Vec::<&[u8]>::new()
        );
    }

    #[test]
    fn test_captures_matches() {
        assert_eq!(
            Captures::new(
                b"You can use iterator".to_vec(),
                vec![Match::new(0, 3), Match::new(4, 7)]
            )
            .matches()
            .collect::<Vec<_>>(),
            vec![b"You", b"can"]
        );

        assert_eq!(
            Captures::new(b"You can use iterator".to_vec(), vec![Match::new(4, 20)])
                .matches()
                .collect::<Vec<_>>(),
            vec![b"can use iterator"]
        );

        assert_eq!(
            Captures::new(b"You can use iterator".to_vec(), vec![])
                .matches()
                .collect::<Vec<_>>(),
            Vec::<&[u8]>::new()
        );
    }

    #[test]
    #[should_panic]
    fn test_captures_into_iter_panics_on_invalid_match() {
        Captures::new(b"Hello World".to_vec(), vec![Match::new(0, 100)])
            .into_iter()
            .for_each(|_| {});
    }
}
