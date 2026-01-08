//! Timestamp formatting utilities
//!
//! Provides standardized, configurable timestamp formats for log output.
//! Supports ISO 8601, RFC 3339, Unix timestamps, and custom formats.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::SystemTime;

/// Standardized timestamp format options
///
/// Supports various timestamp formats commonly used in logging systems
/// and compatible with log aggregation tools (Elasticsearch, Splunk, Loki, etc.)
///
/// # Examples
///
/// ```
/// use rust_logger_system::core::TimestampFormat;
/// use std::time::SystemTime;
///
/// let format = TimestampFormat::Iso8601;
/// let timestamp = format.format_system_time(&SystemTime::now());
/// // Output: "2025-01-08T10:30:45.123Z"
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimestampFormat {
    /// ISO 8601 with milliseconds: `2025-01-08T10:30:45.123Z`
    ///
    /// This is the default format, widely supported by log aggregation systems.
    #[default]
    Iso8601,

    /// ISO 8601 with microseconds: `2025-01-08T10:30:45.123456Z`
    ///
    /// Provides higher precision for ordering concurrent log entries.
    Iso8601Micros,

    /// RFC 3339 format: `2025-01-08T10:30:45+00:00`
    ///
    /// Standard internet timestamp format with timezone offset.
    Rfc3339,

    /// Unix timestamp in seconds: `1736332245`
    ///
    /// Compact format, useful for systems that expect numeric timestamps.
    Unix,

    /// Unix timestamp in milliseconds: `1736332245123`
    ///
    /// High-precision numeric timestamp for ordering concurrent events.
    UnixMillis,

    /// Unix timestamp in microseconds: `1736332245123456`
    ///
    /// Maximum precision numeric timestamp.
    UnixMicros,

    /// Custom strftime format
    ///
    /// Allows specifying any strftime-compatible format string.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_logger_system::core::TimestampFormat;
    ///
    /// // Apache log format
    /// let format = TimestampFormat::Custom("%d/%b/%Y:%H:%M:%S %z".to_string());
    ///
    /// // Simple date only
    /// let format = TimestampFormat::Custom("%Y-%m-%d".to_string());
    /// ```
    Custom(String),
}

impl TimestampFormat {
    /// Format a `DateTime<Utc>` according to this format
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_logger_system::core::TimestampFormat;
    /// use chrono::Utc;
    ///
    /// let format = TimestampFormat::Iso8601;
    /// let timestamp = format.format(&Utc::now());
    /// assert!(timestamp.ends_with('Z'));
    /// ```
    #[must_use]
    pub fn format(&self, datetime: &DateTime<Utc>) -> String {
        match self {
            TimestampFormat::Iso8601 => datetime.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            TimestampFormat::Iso8601Micros => datetime.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
            TimestampFormat::Rfc3339 => datetime.to_rfc3339(),
            TimestampFormat::Unix => datetime.timestamp().to_string(),
            TimestampFormat::UnixMillis => datetime.timestamp_millis().to_string(),
            TimestampFormat::UnixMicros => datetime.timestamp_micros().to_string(),
            TimestampFormat::Custom(format_str) => datetime.format(format_str).to_string(),
        }
    }

    /// Format a `SystemTime` according to this format
    ///
    /// Convenience method that converts `SystemTime` to `DateTime<Utc>` first.
    #[must_use]
    pub fn format_system_time(&self, timestamp: &SystemTime) -> String {
        let datetime: DateTime<Utc> = (*timestamp).into();
        self.format(&datetime)
    }

    /// Check if this is a Unix-based numeric format
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            TimestampFormat::Unix | TimestampFormat::UnixMillis | TimestampFormat::UnixMicros
        )
    }

    /// Get a description of this format
    #[must_use]
    pub fn description(&self) -> &str {
        match self {
            TimestampFormat::Iso8601 => "ISO 8601 with milliseconds (2025-01-08T10:30:45.123Z)",
            TimestampFormat::Iso8601Micros => {
                "ISO 8601 with microseconds (2025-01-08T10:30:45.123456Z)"
            }
            TimestampFormat::Rfc3339 => "RFC 3339 with timezone (2025-01-08T10:30:45+00:00)",
            TimestampFormat::Unix => "Unix timestamp in seconds (1736332245)",
            TimestampFormat::UnixMillis => "Unix timestamp in milliseconds (1736332245123)",
            TimestampFormat::UnixMicros => "Unix timestamp in microseconds (1736332245123456)",
            TimestampFormat::Custom(_) => "Custom strftime format",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_datetime() -> DateTime<Utc> {
        // 2025-01-08 10:30:45.123456 UTC
        Utc.with_ymd_and_hms(2025, 1, 8, 10, 30, 45)
            .single()
            .expect("valid datetime")
            + chrono::Duration::microseconds(123456)
    }

    #[test]
    fn test_iso8601_format() {
        let format = TimestampFormat::Iso8601;
        let result = format.format(&fixed_datetime());
        assert_eq!(result, "2025-01-08T10:30:45.123Z");
    }

    #[test]
    fn test_iso8601_micros_format() {
        let format = TimestampFormat::Iso8601Micros;
        let result = format.format(&fixed_datetime());
        assert_eq!(result, "2025-01-08T10:30:45.123456Z");
    }

    #[test]
    fn test_rfc3339_format() {
        let format = TimestampFormat::Rfc3339;
        let result = format.format(&fixed_datetime());
        // RFC 3339 format includes timezone offset
        assert!(result.starts_with("2025-01-08T10:30:45"));
        assert!(result.contains("+00:00") || result.ends_with('Z'));
    }

    #[test]
    fn test_unix_format() {
        let format = TimestampFormat::Unix;
        let result = format.format(&fixed_datetime());
        // Should be a valid integer
        let parsed: i64 = result.parse().expect("valid unix timestamp");
        assert!(parsed > 0);
    }

    #[test]
    fn test_unix_millis_format() {
        let format = TimestampFormat::UnixMillis;
        let result = format.format(&fixed_datetime());
        let parsed: i64 = result.parse().expect("valid unix millis timestamp");
        // Milliseconds should be larger than seconds
        let unix_result: i64 = TimestampFormat::Unix
            .format(&fixed_datetime())
            .parse()
            .unwrap();
        assert!(parsed > unix_result);
    }

    #[test]
    fn test_unix_micros_format() {
        let format = TimestampFormat::UnixMicros;
        let result = format.format(&fixed_datetime());
        let parsed: i64 = result.parse().expect("valid unix micros timestamp");
        // Microseconds should be larger than milliseconds
        let millis_result: i64 = TimestampFormat::UnixMillis
            .format(&fixed_datetime())
            .parse()
            .unwrap();
        assert!(parsed > millis_result);
    }

    #[test]
    fn test_custom_format() {
        let format = TimestampFormat::Custom("%Y/%m/%d %H:%M".to_string());
        let result = format.format(&fixed_datetime());
        assert_eq!(result, "2025/01/08 10:30");
    }

    #[test]
    fn test_custom_apache_format() {
        let format = TimestampFormat::Custom("%d/%b/%Y:%H:%M:%S +0000".to_string());
        let result = format.format(&fixed_datetime());
        assert_eq!(result, "08/Jan/2025:10:30:45 +0000");
    }

    #[test]
    fn test_default_is_iso8601() {
        assert_eq!(TimestampFormat::default(), TimestampFormat::Iso8601);
    }

    #[test]
    fn test_is_numeric() {
        assert!(!TimestampFormat::Iso8601.is_numeric());
        assert!(!TimestampFormat::Iso8601Micros.is_numeric());
        assert!(!TimestampFormat::Rfc3339.is_numeric());
        assert!(TimestampFormat::Unix.is_numeric());
        assert!(TimestampFormat::UnixMillis.is_numeric());
        assert!(TimestampFormat::UnixMicros.is_numeric());
        assert!(!TimestampFormat::Custom("%Y".to_string()).is_numeric());
    }

    #[test]
    fn test_format_system_time() {
        let format = TimestampFormat::Iso8601;
        let system_time = SystemTime::now();
        let result = format.format_system_time(&system_time);
        // Should produce a valid ISO 8601 string
        assert!(result.ends_with('Z'));
        assert!(result.contains('T'));
    }

    #[test]
    fn test_serialization() {
        let format = TimestampFormat::Iso8601;
        let json = serde_json::to_string(&format).expect("serialize");
        assert_eq!(json, "\"Iso8601\"");

        let custom = TimestampFormat::Custom("%Y-%m-%d".to_string());
        let json = serde_json::to_string(&custom).expect("serialize custom");
        assert!(json.contains("Custom"));
    }

    #[test]
    fn test_deserialization() {
        let format: TimestampFormat =
            serde_json::from_str("\"Iso8601\"").expect("deserialize Iso8601");
        assert_eq!(format, TimestampFormat::Iso8601);

        let format: TimestampFormat =
            serde_json::from_str(r#"{"Custom":"%Y-%m-%d"}"#).expect("deserialize Custom");
        assert_eq!(format, TimestampFormat::Custom("%Y-%m-%d".to_string()));
    }
}

/// Configuration for log formatting
///
/// This struct holds formatting options that can be shared across appenders.
/// It is wrapped in `Arc` for efficient sharing in multi-threaded contexts.
///
/// # Examples
///
/// ```
/// use rust_logger_system::core::{FormatterConfig, TimestampFormat};
///
/// let config = FormatterConfig::new()
///     .with_timestamp_format(TimestampFormat::Iso8601Micros)
///     .with_level_uppercase(false);
/// ```
#[derive(Debug, Clone)]
pub struct FormatterConfig {
    /// Timestamp format for log entries
    pub timestamp_format: TimestampFormat,
    /// Whether to include log level in output
    pub include_level: bool,
    /// Whether to include thread ID in output
    pub include_thread_id: bool,
    /// Whether to include file location (file:line) in output
    pub include_file_location: bool,
    /// Whether to display log level in uppercase (ERROR vs error)
    pub level_uppercase: bool,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            timestamp_format: TimestampFormat::default(),
            include_level: true,
            include_thread_id: true,
            include_file_location: false,
            level_uppercase: true,
        }
    }
}

impl FormatterConfig {
    /// Create a new formatter configuration with default values
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the timestamp format
    #[must_use]
    pub fn with_timestamp_format(mut self, format: TimestampFormat) -> Self {
        self.timestamp_format = format;
        self
    }

    /// Set whether to include log level
    #[must_use]
    pub fn with_include_level(mut self, include: bool) -> Self {
        self.include_level = include;
        self
    }

    /// Set whether to include thread ID
    #[must_use]
    pub fn with_include_thread_id(mut self, include: bool) -> Self {
        self.include_thread_id = include;
        self
    }

    /// Set whether to include file location
    #[must_use]
    pub fn with_include_file_location(mut self, include: bool) -> Self {
        self.include_file_location = include;
        self
    }

    /// Set whether log level should be uppercase
    #[must_use]
    pub fn with_level_uppercase(mut self, uppercase: bool) -> Self {
        self.level_uppercase = uppercase;
        self
    }

    /// Create a custom timestamp format
    ///
    /// # Arguments
    ///
    /// * `format_str` - A strftime-compatible format string
    #[must_use]
    pub fn with_custom_timestamp(mut self, format_str: &str) -> Self {
        self.timestamp_format = TimestampFormat::Custom(format_str.to_string());
        self
    }

    /// Wrap this config in an Arc for sharing across appenders
    #[must_use]
    pub fn shared(self) -> Arc<Self> {
        Arc::new(self)
    }
}

#[cfg(test)]
mod formatter_config_tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FormatterConfig::default();
        assert_eq!(config.timestamp_format, TimestampFormat::Iso8601);
        assert!(config.include_level);
        assert!(config.include_thread_id);
        assert!(!config.include_file_location);
        assert!(config.level_uppercase);
    }

    #[test]
    fn test_builder_pattern() {
        let config = FormatterConfig::new()
            .with_timestamp_format(TimestampFormat::UnixMillis)
            .with_include_level(false)
            .with_include_thread_id(false)
            .with_include_file_location(true)
            .with_level_uppercase(false);

        assert_eq!(config.timestamp_format, TimestampFormat::UnixMillis);
        assert!(!config.include_level);
        assert!(!config.include_thread_id);
        assert!(config.include_file_location);
        assert!(!config.level_uppercase);
    }

    #[test]
    fn test_custom_timestamp() {
        let config = FormatterConfig::new().with_custom_timestamp("%Y/%m/%d");

        assert_eq!(
            config.timestamp_format,
            TimestampFormat::Custom("%Y/%m/%d".to_string())
        );
    }

    #[test]
    fn test_shared_config() {
        let config = FormatterConfig::new()
            .with_timestamp_format(TimestampFormat::Rfc3339)
            .shared();

        // Can clone Arc cheaply
        let config2 = Arc::clone(&config);
        assert_eq!(config.timestamp_format, config2.timestamp_format);
    }
}
