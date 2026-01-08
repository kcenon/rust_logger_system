//! Appender implementations

pub mod console;
pub mod file;
pub mod json;
pub mod network;
pub mod rotating_file;

#[cfg(feature = "async-appenders")]
pub mod async_file;

pub use console::ConsoleAppender;
pub use file::FileAppender;
pub use json::JsonAppender;
pub use network::NetworkAppender;
pub use rotating_file::{RotatingFileAppender, RotationPolicy, RotationStrategy};

#[cfg(feature = "async-appenders")]
pub use async_file::AsyncFileAppender;

// Re-export traits for backward compatibility
pub use crate::core::Appender;
#[cfg(feature = "async-appenders")]
pub use crate::core::AsyncAppender;
