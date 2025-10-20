//! JSON appender for structured logging

use crate::core::{Appender, LogEntry, Result, StructuredLogEntry};
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
}

impl JsonAppender {
    /// Create a new JSON appender
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            writer: BufWriter::new(file),
            pretty: false,
        })
    }

    /// Create a new JSON appender with pretty printing
    pub fn new_pretty<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            writer: BufWriter::new(file),
            pretty: true,
        })
    }

    /// Convert LogEntry to StructuredLogEntry
    fn to_structured(&self, entry: &LogEntry) -> StructuredLogEntry {
        let mut structured = StructuredLogEntry::new(entry.level, &entry.message);

        // Add context if present
        if let Some(context) = &entry.context {
            structured.context = context.clone();
        }

        structured.timestamp = entry.timestamp.timestamp_millis();
        structured
    }
}

impl Appender for JsonAppender {
    fn name(&self) -> &str {
        "json"
    }

    fn append(&mut self, entry: &LogEntry) -> Result<()> {
        let structured = self.to_structured(entry);

        let json = if self.pretty {
            structured.to_json_pretty()?
        } else {
            structured.to_json()?
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
