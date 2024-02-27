use crate::{Captures, Error, Needle};

type Result<T> = std::result::Result<T, Error>;

pub trait Expect {
    fn expect<N>(&mut self, needle: N) -> Result<Captures>
    where
        N: Needle;

    fn check<N>(&mut self, needle: N) -> Result<Captures>
    where
        N: Needle;

    fn is_matched<N>(&mut self, needle: N) -> Result<bool>
    where
        N: Needle;

    /// Send buffer to the stream.
    fn send<B>(&mut self, buf: B) -> Result<()>
    where
        B: AsRef<[u8]>;

    /// Send line to the stream.
    fn send_line<B>(&mut self, buf: B) -> Result<()>
    where
        B: AsRef<[u8]>;
}

impl<T> Expect for &mut T
where
    T: Expect,
{
    fn expect<N>(&mut self, needle: N) -> Result<Captures>
    where
        N: Needle,
    {
        T::expect(self, needle)
    }

    fn check<N>(&mut self, needle: N) -> Result<Captures>
    where
        N: Needle,
    {
        T::check(self, needle)
    }

    fn is_matched<N>(&mut self, needle: N) -> Result<bool>
    where
        N: Needle,
    {
        T::is_matched(self, needle)
    }

    fn send<B>(&mut self, buf: B) -> Result<()>
    where
        B: AsRef<[u8]>,
    {
        T::send(self, buf)
    }

    fn send_line<B>(&mut self, buf: B) -> Result<()>
    where
        B: AsRef<[u8]>,
    {
        T::send_line(self, buf)
    }
}
