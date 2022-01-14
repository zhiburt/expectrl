#[cfg(feature = "async")]
mod async_session;
#[cfg(feature = "async")]
mod async_stream;
#[cfg(not(feature = "async"))]
mod session;
pub mod stream;

#[cfg(feature = "async")]
pub use async_session::Session;

#[cfg(not(feature = "async"))]
pub use session::Session;
