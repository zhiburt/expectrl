#[cfg(feature = "async")]
mod async_session;
#[cfg(feature = "async")]
mod async_stream;
pub mod stream;
mod session;

#[cfg(feature = "async")]
pub use async_session::Session;

#[cfg(not(feature = "async"))]
pub use session::Session;