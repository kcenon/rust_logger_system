//! Async appender trait for non-blocking log output

use super::{error::Result, log_entry::LogEntry};
use async_trait::async_trait;

/// Trait for asynchronous log appenders
///
/// # Example
///
/// ```no_run
/// use rust_logger_system::core::{AsyncAppender, LogEntry, Result};
/// use async_trait::async_trait;
///
/// struct MyAsyncAppender;
///
/// #[async_trait]
/// impl AsyncAppender for MyAsyncAppender {
///     async fn append(&mut self, entry: &LogEntry) -> Result<()> {
///         // Async log writing logic
///         Ok(())
///     }
///
///     async fn flush(&mut self) -> Result<()> {
///         Ok(())
///     }
///
///     fn name(&self) -> &str {
///         "my_async_appender"
///     }
/// }
/// ```
#[async_trait]
pub trait AsyncAppender: Send + Sync {
    /// Append a log entry asynchronously
    async fn append(&mut self, entry: &LogEntry) -> Result<()>;

    /// Flush buffered entries asynchronously
    async fn flush(&mut self) -> Result<()>;

    /// Get the appender name
    fn name(&self) -> &str;
}
