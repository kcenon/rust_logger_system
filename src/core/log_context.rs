//! Structured logging context for key-value fields
//!
//! This module provides:
//! - `LogContext`: Per-entry structured fields
//! - `LoggerContext`: Persistent fields across all log entries
//! - `ContextGuard`: RAII guard for scoped context

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Value type for structured logging fields
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

impl fmt::Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldValue::String(s) => write!(f, "{}", s),
            FieldValue::Int(i) => write!(f, "{}", i),
            FieldValue::Float(fl) => write!(f, "{}", fl),
            FieldValue::Bool(b) => write!(f, "{}", b),
            FieldValue::Null => write!(f, "null"),
        }
    }
}

impl FieldValue {
    /// Convert to serde_json::Value for JSON serialization
    #[must_use]
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            FieldValue::String(s) => serde_json::Value::String(s.clone()),
            FieldValue::Int(i) => serde_json::Value::Number((*i).into()),
            FieldValue::Float(f) => serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            FieldValue::Bool(b) => serde_json::Value::Bool(*b),
            FieldValue::Null => serde_json::Value::Null,
        }
    }
}

impl From<String> for FieldValue {
    fn from(s: String) -> Self {
        FieldValue::String(s)
    }
}

impl From<&str> for FieldValue {
    fn from(s: &str) -> Self {
        FieldValue::String(s.to_string())
    }
}

impl From<i64> for FieldValue {
    fn from(i: i64) -> Self {
        FieldValue::Int(i)
    }
}

impl From<i32> for FieldValue {
    fn from(i: i32) -> Self {
        FieldValue::Int(i as i64)
    }
}

impl From<f64> for FieldValue {
    fn from(f: f64) -> Self {
        FieldValue::Float(f)
    }
}

impl From<bool> for FieldValue {
    fn from(b: bool) -> Self {
        FieldValue::Bool(b)
    }
}

/// Context for structured logging with key-value fields
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogContext {
    fields: HashMap<String, FieldValue>,
}

impl LogContext {
    /// Create a new empty log context
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    /// Add a field to the context
    pub fn with_field<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<FieldValue>,
    {
        self.fields.insert(key.into(), value.into());
        self
    }

    /// Add a field to the context (mutable version)
    pub fn add_field<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<FieldValue>,
    {
        self.fields.insert(key.into(), value.into());
    }

    /// Get all fields
    pub fn fields(&self) -> &HashMap<String, FieldValue> {
        &self.fields
    }

    /// Check if context has any fields
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Format fields as key=value pairs
    pub fn format_fields(&self) -> String {
        self.fields
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl fmt::Display for LogContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_fields())
    }
}

/// Logger-level persistent context for structured logging
///
/// `LoggerContext` stores fields that persist across all log entries.
/// This is useful for adding common fields like service name, version,
/// or environment to every log entry.
///
/// Thread-safe: Can be safely shared across threads.
///
/// # Example
///
/// ```
/// use rust_logger_system::core::LoggerContext;
///
/// let ctx = LoggerContext::new();
/// ctx.set("service", "api-gateway");
/// ctx.set("version", "1.2.3");
///
/// // Later, these fields are automatically merged into log entries
/// let fields = ctx.get_fields();
/// assert_eq!(fields.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct LoggerContext {
    fields: Arc<RwLock<HashMap<String, FieldValue>>>,
}

impl LoggerContext {
    /// Create a new empty logger context
    pub fn new() -> Self {
        Self {
            fields: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set a field in the context
    ///
    /// If the field already exists, it will be overwritten.
    pub fn set<K, V>(&self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<FieldValue>,
    {
        self.fields.write().insert(key.into(), value.into());
    }

    /// Remove a field from the context
    pub fn remove(&self, key: &str) {
        self.fields.write().remove(key);
    }

    /// Clear all fields from the context
    pub fn clear(&self) {
        self.fields.write().clear();
    }

    /// Get a clone of all fields
    pub fn get_fields(&self) -> HashMap<String, FieldValue> {
        self.fields.read().clone()
    }

    /// Check if the context is empty
    pub fn is_empty(&self) -> bool {
        self.fields.read().is_empty()
    }

    /// Get the number of fields in the context
    pub fn len(&self) -> usize {
        self.fields.read().len()
    }

    /// Merge context fields into a LogContext
    ///
    /// Entry-level fields take priority over logger-level fields.
    pub fn merge_into(&self, log_context: &mut LogContext) {
        let fields = self.fields.read();
        for (key, value) in fields.iter() {
            // Only insert if the key doesn't exist (entry-level takes priority)
            if !log_context.fields.contains_key(key) {
                log_context.fields.insert(key.clone(), value.clone());
            }
        }
    }

    /// Create a LogContext from the logger context
    pub fn to_log_context(&self) -> LogContext {
        let fields = self.fields.read();
        LogContext {
            fields: fields.clone(),
        }
    }

    /// Get the internal fields Arc for creating ContextGuard
    ///
    /// This is used internally for creating RAII guards.
    pub(crate) fn inner_fields(&self) -> Arc<RwLock<HashMap<String, FieldValue>>> {
        Arc::clone(&self.fields)
    }
}

impl Default for LoggerContext {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard for scoped context fields
///
/// When dropped, automatically removes the field from the logger context.
/// This is useful for adding temporary context fields for a specific scope.
///
/// # Example
///
/// ```ignore
/// let logger = Logger::builder().build();
///
/// {
///     let _guard = logger.with_context("request_id", "abc-123");
///     logger.info("Processing request");  // Includes request_id
/// }
/// // request_id automatically removed here
/// ```
pub struct ContextGuard {
    context: Arc<RwLock<HashMap<String, FieldValue>>>,
    key: String,
}

impl ContextGuard {
    /// Create a new context guard
    pub(crate) fn new(context: Arc<RwLock<HashMap<String, FieldValue>>>, key: String) -> Self {
        Self { context, key }
    }
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        self.context.write().remove(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_context_creation() {
        let ctx = LogContext::new();
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_log_context_with_fields() {
        let ctx = LogContext::new()
            .with_field("user_id", 123)
            .with_field("username", "john_doe")
            .with_field("active", true);

        assert_eq!(ctx.fields().len(), 3);
        assert!(!ctx.is_empty());
    }

    #[test]
    fn test_log_context_format() {
        let ctx = LogContext::new()
            .with_field("key1", "value1")
            .with_field("key2", 42);

        let formatted = ctx.format_fields();
        assert!(formatted.contains("key1=value1"));
        assert!(formatted.contains("key2=42"));
    }

    #[test]
    fn test_logger_context_basic() {
        let ctx = LoggerContext::new();
        ctx.set("service", "api-gateway");
        ctx.set("version", "1.2.3");

        let fields = ctx.get_fields();
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn test_logger_context_remove() {
        let ctx = LoggerContext::new();
        ctx.set("key1", "value1");
        ctx.set("key2", "value2");

        assert_eq!(ctx.get_fields().len(), 2);

        ctx.remove("key1");
        assert_eq!(ctx.get_fields().len(), 1);
        assert!(!ctx.get_fields().contains_key("key1"));
    }

    #[test]
    fn test_logger_context_clear() {
        let ctx = LoggerContext::new();
        ctx.set("key1", "value1");
        ctx.set("key2", "value2");

        ctx.clear();
        assert!(ctx.get_fields().is_empty());
    }

    #[test]
    fn test_logger_context_merge_into() {
        let logger_ctx = LoggerContext::new();
        logger_ctx.set("service", "api");
        logger_ctx.set("version", "1.0");

        let mut log_ctx = LogContext::new()
            .with_field("user_id", 123);

        logger_ctx.merge_into(&mut log_ctx);

        // Should have both original and merged fields
        assert_eq!(log_ctx.fields().len(), 3);
        assert!(log_ctx.fields().contains_key("service"));
        assert!(log_ctx.fields().contains_key("version"));
        assert!(log_ctx.fields().contains_key("user_id"));
    }

    #[test]
    fn test_logger_context_merge_priority() {
        let logger_ctx = LoggerContext::new();
        logger_ctx.set("key", "logger_value");

        let mut log_ctx = LogContext::new()
            .with_field("key", "entry_value");

        logger_ctx.merge_into(&mut log_ctx);

        // Entry-level field should take priority
        match log_ctx.fields().get("key") {
            Some(FieldValue::String(s)) => assert_eq!(s, "entry_value"),
            _ => panic!("Expected string value"),
        }
    }
}
