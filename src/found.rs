use crate::needle::Match;

/// Found is a represention of a matched pattern.
///
/// It might represent an empty match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    buf: Vec<u8>,
    matches: Vec<Match>,
}

impl Found {
    /// New returns an instance of Found.
    pub(crate) fn new(buf: Vec<u8>, matches: Vec<Match>) -> Self {
        Self { buf, matches }
    }

    /// is_empty verifies if any matches were actually found.
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// Matches returns a list of matches.
    pub fn matches(&self) -> Vec<&[u8]> {
        self.matches
            .iter()
            .map(|m| &self.buf[m.start()..m.end()])
            .collect()
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

impl IntoIterator for Found {
    type Item = Vec<u8>;
    type IntoIter = std::vec::IntoIter<Vec<u8>>;

    fn into_iter(self) -> Self::IntoIter {
        self.matches()
            .into_iter()
            .map(|m| m.to_vec())
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl<'a> IntoIterator for &'a Found {
    type Item = &'a [u8];
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.matches().into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iterator_on_found() {
        assert_eq!(
            Found::new(
                b"You can use iterator".to_vec(),
                vec![Match::new(0, 3), Match::new(4, 7)]
            )
            .into_iter()
            .collect::<Vec<Vec<u8>>>(),
            vec![b"You".to_vec(), b"can".to_vec()]
        );
    }
}
