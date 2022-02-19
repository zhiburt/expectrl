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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iterator_on_found() {
        assert_eq!(
            Captures::new(
                b"You can use iterator".to_vec(),
                vec![Match::new(0, 3), Match::new(4, 7)]
            )
            .into_iter()
            .collect::<Vec<&[u8]>>(),
            vec![b"You".to_vec(), b"can".to_vec()]
        );
    }
}
