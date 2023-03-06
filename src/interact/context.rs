/// Context provides an interface to use a [`Session`], IO streams
/// and a state.
///
/// It's used primarily in callbacks for [`InteractSession`].
///
/// [`InteractSession`]: crate::interact::InteractSession
/// [`Session`]: crate::session::Session
#[derive(Debug)]
pub struct Context<'a, Session, Input, Output, State> {
    /// The field contains a &mut reference to a [`Session`].
    ///
    /// [`Session`]: crate::session::Session
    pub session: &'a mut Session,
    /// The field contains an input structure which was used in [`InteractSession`].
    ///
    /// [`InteractSession`]: crate::interact::InteractSession
    pub input: &'a mut Input,
    /// The field contains an output structure which was used in [`InteractSession`].
    ///
    /// [`InteractSession`]: crate::interact::InteractSession
    pub output: &'a mut Output,
    /// The field contains a user defined data.
    pub state: &'a mut State,
    /// The field contains a bytes which were consumed from a user or the running process.
    pub buf: &'a [u8],
    /// A flag for EOF of a user session or running process.
    pub eof: bool,
}

impl<'a, Session, Input, Output, State> Context<'a, Session, Input, Output, State> {
    /// Creates a new [`Context`] structure.
    pub fn new(
        session: &'a mut Session,
        input: &'a mut Input,
        output: &'a mut Output,
        state: &'a mut State,
        buf: &'a [u8],
        eof: bool,
    ) -> Self {
        Self {
            session,
            input,
            output,
            buf,
            eof,
            state,
        }
    }
}
