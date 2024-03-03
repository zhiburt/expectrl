use crate::{Captures, Error, Needle};

/// Expect trait provides common expect functions.
pub trait Expect {
    /// Expect waits until a pattern is matched.
    ///
    /// If the method returns [Ok] it is guaranteed that at least 1 match was found.
    ///
    /// The match algorthm can be either
    ///     - gready
    ///     - lazy
    ///
    /// You can set one via [Session::set_expect_lazy].
    /// Default version is gready.
    ///
    /// The implications are.
    /// Imagine you use [crate::Regex] `"\d+"` to find a match.
    /// And your process outputs `123`.
    /// In case of lazy approach we will match `1`.
    /// Where's in case of gready one we will match `123`.
    ///
    /// # Example
    ///
    #[cfg_attr(any(windows, feature = "async"), doc = "```ignore")]
    #[cfg_attr(not(any(windows, feature = "async")), doc = "```")]
    /// use expectrl::{Expect, spawn, Regex};
    ///
    /// let mut p = spawn("echo 123").unwrap();
    /// let m = p.expect(Regex("\\d+")).unwrap();
    /// assert_eq!(m.get(0).unwrap(), b"123");
    /// ```
    ///
    #[cfg_attr(any(windows, feature = "async"), doc = "```ignore")]
    #[cfg_attr(not(any(windows, feature = "async")), doc = "```")]
    /// use expectrl::{Expect, spawn, Regex};
    ///
    /// let mut p = spawn("echo 123").unwrap();
    /// p.set_expect_lazy(true);
    /// let m = p.expect(Regex("\\d+")).unwrap();
    /// assert_eq!(m.get(0).unwrap(), b"1");
    /// ```
    ///
    /// This behaviour is different from [Session::check].
    ///
    /// It returns an error if timeout is reached.
    /// You can specify a timeout value by [Session::set_expect_timeout] method.
    fn expect<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle;

    /// Check verifies if a pattern is matched.
    /// Returns empty found structure if nothing found.
    ///
    /// Is a non blocking version of [Session::expect].
    /// But its strategy of matching is different from it.
    /// It makes search against all bytes available.
    ///
    /// # Example
    ///
    #[cfg_attr(
        any(windows, target_os = "macos", feature = "async"),
        doc = "```ignore"
    )]
    #[cfg_attr(not(any(windows, target_os = "macos", feature = "async")), doc = "```")]
    /// use expectrl::{spawn, Regex, Expect};
    /// use std::time::Duration;
    ///
    /// let mut p = spawn("echo 123").unwrap();
    /// #
    /// # // wait to guarantee that check echo worked out (most likely)
    /// # std::thread::sleep(Duration::from_millis(500));
    /// #
    /// let m = p.check(Regex("\\d+")).unwrap();
    /// assert_eq!(m.get(0).unwrap(), b"123");
    /// ```
    fn check<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle;

    /// The functions checks if a pattern is matched.
    /// It doesn’t consumes bytes from stream.
    ///
    /// Its strategy of matching is different from the one in [Session::expect].
    /// It makes search agains all bytes available.
    ///
    /// If you want to get a matched result [Session::check] and [Session::expect] is a better option.
    /// Because it is not guaranteed that [Session::check] or [Session::expect] with the same parameters:
    ///     - will successed even right after Session::is_matched call.
    ///     - will operate on the same bytes.
    ///
    /// IMPORTANT:
    ///  
    /// If you call this method with [crate::Eof] pattern be aware that eof
    /// indication MAY be lost on the next interactions.
    /// It depends from a process you spawn.
    /// So it might be better to use [Session::check] or [Session::expect] with Eof.
    ///
    /// # Example
    ///
    #[cfg_attr(any(windows, feature = "async"), doc = "```ignore")]
    #[cfg_attr(not(any(windows, feature = "async")), doc = "```")]
    /// use expectrl::{spawn, Regex, Expect};
    /// use std::time::Duration;
    ///
    /// let mut p = spawn("cat").unwrap();
    /// p.send_line("123");
    /// # // wait to guarantee that check echo worked out (most likely)
    /// # std::thread::sleep(Duration::from_secs(1));
    /// let m = p.is_matched(Regex("\\d+")).unwrap();
    /// assert_eq!(m, true);
    /// ```
    fn is_matched<N>(&mut self, needle: N) -> Result<bool, Error>
    where
        N: Needle;

    /// Send buffer to the stream.
    ///
    /// You may also use methods from [std::io::Write] instead.
    ///
    /// # Example
    ///
    #[cfg_attr(any(windows, feature = "async"), doc = "```ignore")]
    #[cfg_attr(not(any(windows, feature = "async")), doc = "```")]
    /// use expectrl::{spawn, ControlCode, Expect};
    ///
    /// let mut proc = spawn("cat").unwrap();
    ///
    /// proc.send("Hello");
    /// proc.send(b"World");
    /// proc.send(ControlCode::try_from("^C").unwrap());
    /// ```
    fn send<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>;

    /// Send line to the stream.
    ///
    /// # Example
    ///
    #[cfg_attr(any(windows, feature = "async"), doc = "```ignore")]
    #[cfg_attr(not(any(windows, feature = "async")), doc = "```")]
    /// use expectrl::{spawn, ControlCode, Expect};
    ///
    /// let mut proc = spawn("cat").unwrap();
    ///
    /// proc.send_line("Hello");
    /// proc.send_line(b"World");
    /// proc.send_line(ControlCode::try_from("^C").unwrap());
    /// ```
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
/// Expect trait provides common expect functions.
pub trait AsyncExpect {
    /// Expect waits until a pattern is matched.
    ///
    /// If the method returns [Ok] it is guaranteed that at least 1 match was found.
    ///
    /// The match algorthm can be either
    ///     - gready
    ///     - lazy
    ///
    /// You can set one via [Session::set_expect_lazy].
    /// Default version is gready.
    ///
    /// The implications are.
    ///
    /// Imagine you use [crate::Regex] `"\d+"` to find a match.
    /// And your process outputs `123`.
    /// In case of lazy approach we will match `1`.
    /// Where's in case of gready one we will match `123`.
    ///
    /// # Example
    ///
    #[cfg_attr(windows, doc = "```no_run")]
    #[cfg_attr(unix, doc = "```")]
    /// # futures_lite::future::block_on(async {
    /// use expectrl::{AsyncExpect, spawn, Regex};
    ///
    /// let mut p = spawn("echo 123").unwrap();
    /// let m = p.expect(Regex("\\d+")).await.unwrap();
    /// assert_eq!(m.get(0).unwrap(), b"123");
    /// # });
    /// ```
    ///
    #[cfg_attr(windows, doc = "```no_run")]
    #[cfg_attr(unix, doc = "```")]
    /// # futures_lite::future::block_on(async {
    /// use expectrl::{AsyncExpect, spawn, Regex};
    ///
    /// let mut p = spawn("echo 123").unwrap();
    /// p.set_expect_lazy(true);
    /// let m = p.expect(Regex("\\d+")).await.unwrap();
    /// assert_eq!(m.get(0).unwrap(), b"1");
    /// # });
    /// ```
    ///
    /// This behaviour is different from [Session::check].
    ///
    /// It returns an error if timeout is reached.
    /// You can specify a timeout value by [Session::set_expect_timeout] method.
    async fn expect<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle;

    /// Check checks if a pattern is matched.
    /// Returns empty found structure if nothing found.
    ///
    /// Is a non blocking version of [Session::expect].
    /// But its strategy of matching is different from it.
    /// It makes search agains all bytes available.
    ///
    #[cfg_attr(any(target_os = "macos", windows), doc = "```no_run")]
    #[cfg_attr(not(any(target_os = "macos", windows)), doc = "```")]
    /// # futures_lite::future::block_on(async {
    /// use expectrl::{AsyncExpect, spawn, Regex};
    ///
    /// let mut p = spawn("echo 123").unwrap();
    /// // wait to guarantee that check will successed (most likely)
    /// std::thread::sleep(std::time::Duration::from_secs(1));
    /// let m = p.check(Regex("\\d+")).await.unwrap();
    /// assert_eq!(m.get(0).unwrap(), b"123");
    /// # });
    /// ```
    async fn check<N>(&mut self, needle: N) -> Result<Captures, Error>
    where
        N: Needle;

    /// Is matched checks if a pattern is matched.
    /// It doesn't consumes bytes from stream.
    async fn is_matched<N>(&mut self, needle: N) -> Result<bool, Error>
    where
        N: Needle;

    /// Send text to child’s STDIN.
    ///
    /// You can also use methods from [std::io::Write] instead.
    ///
    /// # Example
    ///
    /// ```
    /// use expectrl::{spawn, ControlCode, AsyncExpect};
    ///
    /// let mut proc = spawn("cat").unwrap();
    ///
    /// # futures_lite::future::block_on(async {
    /// proc.send("Hello").await;
    /// proc.send(b"World").await;
    /// proc.send(ControlCode::try_from("^C").unwrap()).await;
    /// # });
    /// ```
    async fn send<B>(&mut self, buf: B) -> Result<(), Error>
    where
        B: AsRef<[u8]>;

    /// Send a line to child’s STDIN.
    ///
    /// # Example
    ///
    /// ```
    /// use expectrl::{spawn, ControlCode, AsyncExpect};
    ///
    /// let mut proc = spawn("cat").unwrap();
    ///
    /// # futures_lite::future::block_on(async {
    /// proc.send_line("Hello").await;
    /// proc.send_line(b"World").await;
    /// proc.send_line(ControlCode::try_from("^C").unwrap()).await;
    /// # });
    /// ```
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
