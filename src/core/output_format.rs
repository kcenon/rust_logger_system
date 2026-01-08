//! Output format configuration for log entries
//!
//! Provides different output formats for log entries:
//! - Text: Human-readable format (default)
//! - Json: Machine-readable JSON format
//! - Logfmt: Key-value format compatible with log aggregation tools

use super::log_entry::LogEntry;
use super::timestamp::TimestampFormat;

/// Output format for log entries
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text format (default)
    ///
    /// Example: `[2025-01-08T10:30:45Z] [INFO ] main - Request processed`
    #[default]
    Text,

    /// JSON format for machine processing
    ///
    /// Example: `{"timestamp":"2025-01-08T10:30:45Z","level":"INFO","message":"Request processed"}`
    Json,

    /// Logfmt format (key=value pairs)
    ///
    /// Example: `timestamp=2025-01-08T10:30:45Z level=INFO message="Request processed"`
    Logfmt,
}

impl OutputFormat {
    /// Format a log entry according to this output format
    pub fn format(&self, entry: &LogEntry, timestamp_format: &TimestampFormat) -> String {
        match self {
            OutputFormat::Text => self.format_text(entry, timestamp_format),
            OutputFormat::Json => self.format_json(entry, timestamp_format),
            OutputFormat::Logfmt => self.format_logfmt(entry, timestamp_format),
        }
    }

    /// Format as human-readable text
    fn format_text(&self, entry: &LogEntry, timestamp_format: &TimestampFormat) -> String {
        let timestamp_str = timestamp_format.format(&entry.timestamp);
        let thread_name = entry.thread_name.as_ref().unwrap_or(&entry.thread_id);

        let base = format!(
            "[{}] [{:5}] {} - {}",
            timestamp_str,
            entry.level.to_str(),
            thread_name,
            entry.message
        );

        // Append context fields if present
        if let Some(ref context) = entry.context {
            if !context.is_empty() {
                return format!("{} {}", base, context.format_fields());
            }
        }

        base
    }

    /// Format as JSON
    fn format_json(&self, entry: &LogEntry, timestamp_format: &TimestampFormat) -> String {
        let mut json_obj = serde_json::Map::new();

        // Add timestamp
        json_obj.insert(
            "timestamp".to_string(),
            self.format_timestamp_json(entry, timestamp_format),
        );

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
        if let Some(ref name) = entry.thread_name {
            json_obj.insert(
                "thread_name".to_string(),
                serde_json::Value::String(name.clone()),
            );
        }

        // Add location info if present
        if let Some(ref file) = entry.file {
            json_obj.insert("file".to_string(), serde_json::Value::String(file.clone()));
        }
        if let Some(line) = entry.line {
            json_obj.insert("line".to_string(), serde_json::Value::Number(line.into()));
        }
        if let Some(ref module_path) = entry.module_path {
            json_obj.insert(
                "module_path".to_string(),
                serde_json::Value::String(module_path.clone()),
            );
        }

        // Add context fields if present
        if let Some(ref context) = entry.context {
            for (key, value) in context.fields() {
                json_obj.insert(key.clone(), value.to_json_value());
            }
        }

        serde_json::to_string(&serde_json::Value::Object(json_obj)).unwrap_or_default()
    }

    /// Format timestamp for JSON output
    fn format_timestamp_json(
        &self,
        entry: &LogEntry,
        timestamp_format: &TimestampFormat,
    ) -> serde_json::Value {
        match timestamp_format {
            TimestampFormat::Unix => {
                serde_json::Value::Number(entry.timestamp.timestamp().into())
            }
            TimestampFormat::UnixMillis => {
                serde_json::Value::Number(entry.timestamp.timestamp_millis().into())
            }
            TimestampFormat::UnixMicros => {
                serde_json::Value::Number(entry.timestamp.timestamp_micros().into())
            }
            _ => serde_json::Value::String(timestamp_format.format(&entry.timestamp)),
        }
    }

    /// Format as logfmt (key=value pairs)
    fn format_logfmt(&self, entry: &LogEntry, timestamp_format: &TimestampFormat) -> String {
        let mut parts = Vec::new();

        // Add timestamp
        parts.push(format!(
            "timestamp={}",
            self.escape_logfmt_value(&timestamp_format.format(&entry.timestamp))
        ));

        // Add level
        parts.push(format!("level={}", entry.level.to_str()));

        // Add message (always quoted for safety)
        parts.push(format!("message={}", self.quote_logfmt_value(&entry.message)));

        // Add thread info
        parts.push(format!(
            "thread_id={}",
            self.escape_logfmt_value(&entry.thread_id)
        ));
        if let Some(ref name) = entry.thread_name {
            parts.push(format!(
                "thread_name={}",
                self.escape_logfmt_value(name)
            ));
        }

        // Add location info if present
        if let Some(ref file) = entry.file {
            parts.push(format!("file={}", self.escape_logfmt_value(file)));
        }
        if let Some(line) = entry.line {
            parts.push(format!("line={}", line));
        }
        if let Some(ref module_path) = entry.module_path {
            parts.push(format!(
                "module_path={}",
                self.escape_logfmt_value(module_path)
            ));
        }

        // Add context fields if present
        if let Some(ref context) = entry.context {
            for (key, value) in context.fields() {
                let formatted_value = match value {
                    super::log_context::FieldValue::String(s) => self.quote_logfmt_value(s),
                    super::log_context::FieldValue::Int(i) => i.to_string(),
                    super::log_context::FieldValue::Float(f) => f.to_string(),
                    super::log_context::FieldValue::Bool(b) => b.to_string(),
                    super::log_context::FieldValue::Null => "null".to_string(),
                };
                parts.push(format!("{}={}", self.escape_logfmt_key(key), formatted_value));
            }
        }

        parts.join(" ")
    }

    /// Escape a logfmt key (remove spaces and special chars)
    fn escape_logfmt_key(&self, key: &str) -> String {
        key.chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect()
    }

    /// Escape a logfmt value (quote if contains spaces)
    fn escape_logfmt_value(&self, value: &str) -> String {
        if value.contains(' ') || value.contains('"') || value.contains('=') {
            self.quote_logfmt_value(value)
        } else {
            value.to_string()
        }
    }

    /// Quote a logfmt value
    fn quote_logfmt_value(&self, value: &str) -> String {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{LogContext, LogLevel};

    #[test]
    fn test_text_format() {
        let entry = LogEntry::new(LogLevel::Info, "Test message".to_string());
        let format = OutputFormat::Text;
        let result = format.format(&entry, &TimestampFormat::Iso8601);

        assert!(result.contains("INFO"));
        assert!(result.contains("Test message"));
    }

    #[test]
    fn test_text_format_with_context() {
        let context = LogContext::new()
            .with_field("user_id", 123)
            .with_field("action", "login");

        let entry =
            LogEntry::new(LogLevel::Info, "User logged in".to_string()).with_context(context);

        let format = OutputFormat::Text;
        let result = format.format(&entry, &TimestampFormat::Iso8601);

        assert!(result.contains("User logged in"));
        assert!(result.contains("user_id=123"));
        assert!(result.contains("action=login"));
    }

    #[test]
    fn test_json_format() {
        let entry = LogEntry::new(LogLevel::Error, "Error occurred".to_string());
        let format = OutputFormat::Json;
        let result = format.format(&entry, &TimestampFormat::Iso8601);

        // Parse as JSON to verify structure
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["level"], "ERROR");
        assert_eq!(parsed["message"], "Error occurred");
        assert!(parsed["timestamp"].is_string());
    }

    #[test]
    fn test_json_format_with_context() {
        let context = LogContext::new()
            .with_field("request_id", "abc-123")
            .with_field("latency_ms", 42);

        let entry =
            LogEntry::new(LogLevel::Info, "Request completed".to_string()).with_context(context);

        let format = OutputFormat::Json;
        let result = format.format(&entry, &TimestampFormat::Iso8601);

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["request_id"], "abc-123");
        assert_eq!(parsed["latency_ms"], 42);
    }

    #[test]
    fn test_logfmt_format() {
        let entry = LogEntry::new(LogLevel::Warn, "Warning message".to_string());
        let format = OutputFormat::Logfmt;
        let result = format.format(&entry, &TimestampFormat::Iso8601);

        assert!(result.contains("level=WARN"));
        assert!(result.contains("message=\"Warning message\""));
    }

    #[test]
    fn test_logfmt_format_with_context() {
        let context = LogContext::new()
            .with_field("user", "alice")
            .with_field("count", 5);

        let entry =
            LogEntry::new(LogLevel::Debug, "Debug info".to_string()).with_context(context);

        let format = OutputFormat::Logfmt;
        let result = format.format(&entry, &TimestampFormat::Iso8601);

        assert!(result.contains("user=\"alice\"") || result.contains("user=alice"));
        assert!(result.contains("count=5"));
    }

    #[test]
    fn test_logfmt_escape_special_chars() {
        let context = LogContext::new().with_field("query", "SELECT * FROM users WHERE id=1");

        let entry =
            LogEntry::new(LogLevel::Debug, "Query executed".to_string()).with_context(context);

        let format = OutputFormat::Logfmt;
        let result = format.format(&entry, &TimestampFormat::Iso8601);

        // Value with = should be quoted
        assert!(result.contains("query=\"SELECT * FROM users WHERE id=1\""));
    }

    #[test]
    fn test_output_format_default() {
        let format = OutputFormat::default();
        assert_eq!(format, OutputFormat::Text);
    }
}
