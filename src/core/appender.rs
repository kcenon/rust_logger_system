//! Appender trait for log output destinations

use super::{error::Result, log_entry::LogEntry};

pub trait Appender: Send + Sync {
    fn append(&mut self, entry: &LogEntry) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
    fn name(&self) -> &str;
}
