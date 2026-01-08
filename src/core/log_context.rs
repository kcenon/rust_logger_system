//! Structured logging context for key-value fields

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

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
}
