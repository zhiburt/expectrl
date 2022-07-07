/// Context provides an interface to use a [`Session`], IO streams
/// and a state.
///
/// It's used primarily in callbacks for [`InteractSession`].
///
/// [`InteractSession`]: crate::interact::InteractSession
/// [`Session`]: crate::session::Session
pub struct Context<'a, Session, Output, State> {
    /// The field contains a &mut reference to a [`Session`].
    ///
    /// [`Session`]: crate::session::Session
    pub session: Session,
    /// The field contains an output structure which was used in [`InteractSession`].
    ///
    /// [`InteractSession`]: crate::interact::InteractSession
    pub output: Output,
    /// The field contains a bytes which were consumed from a user or the running process.
    pub buf: &'a [u8],
    /// A flag for EOF of a user session or running process.
    pub eof: bool,
    /// The field contains a user defined data.
    pub state: State,
}

impl<'a, Session, Output, State> Context<'a, Session, Output, State> {
    /// Creates a new [`Context`] structure.
    pub fn new(session: Session, output: Output, buf: &'a [u8], eof: bool, state: State) -> Self {
        Self {
            session,
            output,
            buf,
            eof,
            state,
        }
    }
}
