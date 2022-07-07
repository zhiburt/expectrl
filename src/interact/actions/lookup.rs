//! The module contains [`Lookup`].

use crate::{Captures, Error, Needle};

/// A helper action for an [`InteractSession`]
///
/// It holds a buffer to be able to run different checks using [`Needle`],
/// via [`Lookup::on`].
///
/// [`InteractSession`]: crate::interact::InteractSession
#[derive(Debug, Clone)]
pub struct Lookup {
    buf: Vec<u8>,
}

impl Lookup {
    /// Create a lookup object.
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Checks whethere the buffer will be matched and returns [`Captures`] in such case.
    pub fn on<N>(&mut self, buf: &[u8], eof: bool, pattern: N) -> Result<Option<Captures>, Error>
    where
        N: Needle,
    {
        self.buf.extend(buf);
        check(&mut self.buf, pattern, eof)
    }

    /// Cleans internal buffer.
    ///
    /// So the next [`Lookup::on`] can be called with no other data involved.
    pub fn clear(&mut self) {
        self.buf.clear();
    }
}

impl Default for Lookup {
    fn default() -> Self {
        Self::new()
    }
}

fn check<N>(buf: &mut Vec<u8>, needle: N, eof: bool) -> Result<Option<Captures>, Error>
where
    N: Needle,
{
    // we ignore the check if buf is empty in just in case someone is matching 0 bytes.

    let found = needle.check(buf, eof)?;
    if found.is_empty() {
        return Ok(None);
    }

    let end_index = Captures::right_most_index(&found);
    let involved_bytes = buf[..end_index].to_vec();
    let found = Captures::new(involved_bytes, found);
    let _ = buf.drain(..end_index);

    Ok(Some(found))
}
