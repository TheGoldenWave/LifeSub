pub(crate) mod committer;
pub(crate) mod coordinator;
pub mod helper;
pub mod helper_auth;
pub(crate) mod native_transport;
pub mod protocol;
#[allow(dead_code)] // Task 8 wires this capability into the production coordinator.
pub(crate) mod seal;
#[cfg(feature = "desktop")]
pub(crate) mod session;

#[cfg(feature = "desktop")]
pub mod streaming;

#[cfg(feature = "desktop")]
pub use streaming::StreamingCapture;
