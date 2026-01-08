//! JSON appender for structured logging

use crate::core::{Appender, LogEntry, Result, TimestampFormat};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

/// JSON file appender for structured logging
///
/// Writes each log entry as a single-line JSON object (JSONL format)
/// Compatible with log aggregation tools like ELK, Loki, etc.
pub struct JsonAppender {
    writer: BufWriter<File>,
    pretty: bool,
    timestamp_format: TimestampFormat,
}

impl JsonAppender {
    /// Create a new JSON appender
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;

        Ok(Self {
            writer: BufWriter::new(file),
            pretty: false,
            timestamp_format: TimestampFormat::default(),
        })
    }

    /// Create a new JSON appender with pretty printing
    pub fn new_pretty<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;

        Ok(Self {
            writer: BufWriter::new(file),
            pretty: true,
            timestamp_format: TimestampFormat::default(),
        })
    }

    /// Set the timestamp format for this appender
    ///
    /// For JSON output:
    /// - Numeric formats (Unix, UnixMillis, UnixMicros) output a number
    /// - String formats (Iso8601, Rfc3339, Custom) output a string
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_logger_system::appenders::JsonAppender;
    /// use rust_logger_system::TimestampFormat;
    ///
    /// let appender = JsonAppender::new("/var/log/app.jsonl")
    ///     .unwrap()
    ///     .with_timestamp_format(TimestampFormat::Iso8601);
    /// ```
    #[must_use]
    pub fn with_timestamp_format(mut self, format: TimestampFormat) -> Self {
        self.timestamp_format = format;
        self
    }

    /// Set a custom timestamp format using a strftime-compatible format string
    #[must_use]
    pub fn with_custom_timestamp(mut self, format_str: &str) -> Self {
        self.timestamp_format = TimestampFormat::Custom(format_str.to_string());
        self
    }

    /// Format timestamp according to configured format
    fn format_timestamp(&self, entry: &LogEntry) -> serde_json::Value {
        match &self.timestamp_format {
            TimestampFormat::Unix => {
                serde_json::Value::Number(entry.timestamp.timestamp().into())
            }
            TimestampFormat::UnixMillis => {
                serde_json::Value::Number(entry.timestamp.timestamp_millis().into())
            }
            TimestampFormat::UnixMicros => {
                serde_json::Value::Number(entry.timestamp.timestamp_micros().into())
            }
            _ => serde_json::Value::String(self.timestamp_format.format(&entry.timestamp)),
        }
    }
}

impl Appender for JsonAppender {
    fn name(&self) -> &str {
        "json"
    }

    fn append(&mut self, entry: &LogEntry) -> Result<()> {
        // Build JSON object with configurable timestamp format
        let mut json_obj = serde_json::Map::new();

        // Add timestamp with configured format
        json_obj.insert("timestamp".to_string(), self.format_timestamp(entry));

        // Add level
        json_obj.insert(
            "level".to_string(),
            serde_json::Value::String(entry.level.to_str().to_string()),
        );

        // Add message
        json_obj.insert(
            "message".to_string(),
            serde_json::Value::String(entry.message.clone()),
        );

        // Add thread info
        json_obj.insert(
            "thread_id".to_string(),
            serde_json::Value::String(entry.thread_id.clone()),
        );
        if let Some(name) = &entry.thread_name {
            json_obj.insert(
                "thread_name".to_string(),
                serde_json::Value::String(name.clone()),
            );
        }

        // Add context fields if present
        if let Some(context) = &entry.context {
            for (key, value) in context.fields() {
                json_obj.insert(key.clone(), value.to_json_value());
            }
        }

        // Serialize to JSON
        let json_value = serde_json::Value::Object(json_obj);
        let json = if self.pretty {
            serde_json::to_string_pretty(&json_value)?
        } else {
            serde_json::to_string(&json_value)?
        };

        writeln!(self.writer, "{}", json)?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{LogContext, LogLevel};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_json_appender() -> Result<()> {
        let dir = tempdir()?;
        let log_path = dir.path().join("test.jsonl");

        let mut appender = JsonAppender::new(&log_path)?;

        let context = LogContext::new()
            .with_field("user_id", 123)
            .with_field("action", "login");

        let entry = LogEntry::new(LogLevel::Info, "User logged in".to_string())
            .with_context(context);

        appender.append(&entry)?;
        appender.flush()?;

        let content = fs::read_to_string(&log_path)?;
        assert!(content.contains("User logged in"));
        assert!(content.contains("user_id"));
        assert!(content.contains("123"));

        Ok(())
    }

    #[test]
    fn test_json_appender_multiple_entries() -> Result<()> {
        let dir = tempdir()?;
        let log_path = dir.path().join("test_multiple.jsonl");

        let mut appender = JsonAppender::new(&log_path)?;

        for i in 0..5 {
            let context = LogContext::new()
                .with_field("iteration", i);

            let entry = LogEntry::new(LogLevel::Debug, format!("Iteration {}", i))
                .with_context(context);

            appender.append(&entry)?;
        }

        appender.flush()?;

        let content = fs::read_to_string(&log_path)?;
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 5);

        // Each line should be valid JSON
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line)?;
            assert!(parsed["message"].is_string());
            assert!(parsed["level"].is_string());
        }

        Ok(())
    }
}
