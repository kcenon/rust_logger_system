//! Structured log entry with distributed tracing support

use super::log_context::LogContext;
use super::log_level::LogLevel;
use serde::{Deserialize, Serialize};

/// Tracing context for distributed tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingContext {
    /// Trace ID for request correlation
    pub trace_id: String,

    /// Span ID for this operation
    pub span_id: String,

    /// Parent span ID (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
}

impl TracingContext {
    /// Create a new tracing context
    pub fn new(trace_id: String, span_id: String) -> Self {
        Self {
            trace_id,
            span_id,
            parent_span_id: None,
        }
    }

    /// Set parent span ID
    pub fn with_parent(mut self, parent_span_id: String) -> Self {
        self.parent_span_id = Some(parent_span_id);
        self
    }
}

/// Structured log entry with distributed tracing support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredLogEntry {
    /// Log timestamp in milliseconds since epoch
    pub timestamp: i64,

    /// Log level
    pub level: LogLevel,

    /// Log message
    pub message: String,

    /// Structured fields (uses existing LogContext)
    #[serde(flatten)]
    pub context: LogContext,

    /// Optional tracing context for distributed tracing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracing: Option<TracingContext>,
}

impl StructuredLogEntry {
    /// Create a new structured log entry
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now().timestamp_millis(),
            level,
            message: message.into(),
            context: LogContext::new(),
            tracing: None,
        }
    }

    /// Create from existing LogContext
    pub fn from_context(level: LogLevel, message: impl Into<String>, context: LogContext) -> Self {
        Self {
            timestamp: chrono::Utc::now().timestamp_millis(),
            level,
            message: message.into(),
            context,
            tracing: None,
        }
    }

    /// Add tracing context
    pub fn with_tracing(mut self, tracing: TracingContext) -> Self {
        self.tracing = Some(tracing);
        self
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serialize to pretty JSON string
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structured_entry_creation() {
        let context = LogContext::new()
            .with_field("user_id", 12345_i64)
            .with_field("username", "alice")
            .with_field("success", true);

        let tracing = TracingContext::new("trace-123".to_string(), "span-456".to_string());

        let entry = StructuredLogEntry::from_context(LogLevel::Info, "Test message", context)
            .with_tracing(tracing);

        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.message, "Test message");
        assert!(entry.tracing.is_some());
    }

    #[test]
    fn test_json_serialization() {
        let context = LogContext::new()
            .with_field("query", "SELECT * FROM users")
            .with_field("error_code", 500_i32);

        let entry = StructuredLogEntry::from_context(LogLevel::Error, "Database error", context);

        let json = entry.to_json().unwrap();
        assert!(json.contains("Database error"));
        assert!(json.contains("error_code"));
        assert!(json.contains("500"));
    }

    #[test]
    fn test_json_roundtrip() {
        let context = LogContext::new()
            .with_field("retry_count", 3_i32);

        let entry = StructuredLogEntry::from_context(LogLevel::Warn, "Warning message", context);

        let json = entry.to_json().unwrap();
        let deserialized = StructuredLogEntry::from_json(&json).unwrap();

        assert_eq!(deserialized.level, LogLevel::Warn);
        assert_eq!(deserialized.message, "Warning message");
    }

    #[test]
    fn test_tracing_context() {
        let tracing = TracingContext::new("trace-abc".to_string(), "span-123".to_string())
            .with_parent("span-000".to_string());

        assert_eq!(tracing.trace_id, "trace-abc");
        assert_eq!(tracing.span_id, "span-123");
        assert_eq!(tracing.parent_span_id, Some("span-000".to_string()));
    }
}
