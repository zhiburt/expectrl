use crate::{Captures, Error, Needle};

pub trait Expect {
    fn expect<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle;

    fn check<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle;

    fn is_matched<N>(&mut self, needle: N) -> Result<bool, Error>
    where
        N: Needle;

    /// Send buffer to the stream.
    fn send<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>;

    /// Send line to the stream.
    fn send_line<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>;
}

impl<T> Expect for &mut T
where
    T: Expect,
{
    fn expect<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle,
    {
        T::expect(self, needle)
    }

    fn check<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle,
    {
        T::check(self, needle)
    }

    fn is_matched<N>(&mut self, needle: N) -> Result<bool, Error>
    where
        N: Needle,
    {
        T::is_matched(self, needle)
    }

    fn send<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>,
    {
        T::send(self, buf)
    }

    fn send_line<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>,
    {
        T::send_line(self, buf)
    }
}

#[cfg(feature = "async")]
pub trait AsyncExpect {
    async fn expect<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle;

    async fn check<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle;

    async fn is_matched<N>(&mut self, needle: N) -> Result<bool, Error>
    where
        N: Needle;

    async fn send<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>;

    async fn send_line<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>;
}

#[cfg(feature = "async")]
impl<T> AsyncExpect for &mut T
where
    T: AsyncExpect,
{
    async fn expect<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle,
    {
        T::expect(self, needle).await
    }

    async fn check<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle,
    {
        T::check(self, needle).await
    }

    async fn is_matched<N>(&mut self, needle: N) -> Result<bool, Error>
    where
        N: Needle,
    {
        T::is_matched(self, needle).await
    }

    async fn send<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>,
    {
        T::send(self, buf).await
    }

    async fn send_line<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>,
    {
        T::send_line(self, buf).await
    }
}
