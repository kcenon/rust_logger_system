//! Core logger types and traits

pub mod appender;
pub mod async_appender;
pub mod error;
pub mod log_context;
pub mod log_entry;
pub mod log_level;
pub mod logger;
pub mod metrics;
pub mod overflow_policy;
pub mod structured_entry;
pub mod timestamp;

pub use appender::Appender;
pub use async_appender::AsyncAppender;
pub use error::{LoggerError, Result};
pub use log_context::{FieldValue, LogContext};
pub use log_entry::LogEntry;
pub use log_level::LogLevel;
pub use logger::{Logger, LoggerBuilder, DEFAULT_SHUTDOWN_TIMEOUT};
pub use metrics::LoggerMetrics;
pub use overflow_policy::{LogPriority, OverflowCallback, OverflowPolicy};
pub use structured_entry::{StructuredLogEntry, TracingContext};
pub use timestamp::{FormatterConfig, TimestampFormat};
