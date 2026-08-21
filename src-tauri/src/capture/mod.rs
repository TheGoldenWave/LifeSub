pub mod protocol;

#[cfg(feature = "desktop")]
pub mod streaming;

#[cfg(feature = "desktop")]
pub use streaming::StreamingCapture;
