//! Structured log builder for fluent log entry construction
//!
//! Provides a builder pattern for creating log entries with structured fields.

use super::log_context::{FieldValue, LogContext};
use super::log_level::LogLevel;
use super::logger::Logger;

/// Builder for structured log entries
///
/// Provides a fluent API for constructing log entries with structured fields.
///
/// # Example
///
/// ```
/// use rust_logger_system::prelude::*;
///
/// let logger = Logger::new();
///
/// logger.info_builder()
///     .message("Request processed")
///     .field("user_id", 12345)
///     .field("latency_ms", 42.5)
///     .field("status", 200)
///     .log();
/// ```
pub struct StructuredLogBuilder<'a> {
    logger: &'a Logger,
    level: LogLevel,
    message: String,
    context: LogContext,
    file: Option<&'static str>,
    line: Option<u32>,
    module_path: Option<&'static str>,
}

impl<'a> StructuredLogBuilder<'a> {
    /// Create a new structured log builder
    pub fn new(logger: &'a Logger, level: LogLevel) -> Self {
        Self {
            logger,
            level,
            message: String::new(),
            context: LogContext::new(),
            file: None,
            line: None,
            module_path: None,
        }
    }

    /// Set the log message
    #[must_use]
    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = msg.into();
        self
    }

    /// Add a structured field to the log entry
    #[must_use]
    pub fn field<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<FieldValue>,
    {
        self.context.add_field(key, value);
        self
    }

    /// Add multiple fields from a LogContext
    #[must_use]
    pub fn fields(mut self, context: LogContext) -> Self {
        for (key, value) in context.fields().iter() {
            self.context.add_field(key.clone(), value.clone());
        }
        self
    }

    /// Set source location information
    #[must_use]
    pub fn location(mut self, file: &'static str, line: u32, module_path: &'static str) -> Self {
        self.file = Some(file);
        self.line = Some(line);
        self.module_path = Some(module_path);
        self
    }

    /// Build and send the log entry
    ///
    /// This consumes the builder and logs the entry.
    pub fn log(self) {
        self.logger.log_with_context(self.level, self.message, self.context);
    }
}

impl Logger {
    /// Create a trace-level structured log builder
    ///
    /// # Example
    ///
    /// ```
    /// use rust_logger_system::Logger;
    ///
    /// let logger = Logger::new();
    /// logger.trace_builder()
    ///     .message("Detailed trace")
    ///     .field("variable", "value")
    ///     .log();
    /// ```
    pub fn trace_builder(&self) -> StructuredLogBuilder<'_> {
        StructuredLogBuilder::new(self, LogLevel::Trace)
    }

    /// Create a debug-level structured log builder
    ///
    /// # Example
    ///
    /// ```
    /// use rust_logger_system::Logger;
    ///
    /// let logger = Logger::new();
    /// logger.debug_builder()
    ///     .message("Debug information")
    ///     .field("state", "initialized")
    ///     .log();
    /// ```
    pub fn debug_builder(&self) -> StructuredLogBuilder<'_> {
        StructuredLogBuilder::new(self, LogLevel::Debug)
    }

    /// Create an info-level structured log builder
    ///
    /// # Example
    ///
    /// ```
    /// use rust_logger_system::Logger;
    ///
    /// let logger = Logger::new();
    /// logger.info_builder()
    ///     .message("Request processed")
    ///     .field("user_id", 12345)
    ///     .field("latency_ms", 42.5)
    ///     .log();
    /// ```
    pub fn info_builder(&self) -> StructuredLogBuilder<'_> {
        StructuredLogBuilder::new(self, LogLevel::Info)
    }

    /// Create a warn-level structured log builder
    ///
    /// # Example
    ///
    /// ```
    /// use rust_logger_system::Logger;
    ///
    /// let logger = Logger::new();
    /// logger.warn_builder()
    ///     .message("Resource usage high")
    ///     .field("cpu_percent", 85.5)
    ///     .field("threshold", 80.0)
    ///     .log();
    /// ```
    pub fn warn_builder(&self) -> StructuredLogBuilder<'_> {
        StructuredLogBuilder::new(self, LogLevel::Warn)
    }

    /// Create an error-level structured log builder
    ///
    /// # Example
    ///
    /// ```
    /// use rust_logger_system::Logger;
    ///
    /// let logger = Logger::new();
    /// logger.error_builder()
    ///     .message("Database connection failed")
    ///     .field("error_code", "DB_CONN_TIMEOUT")
    ///     .field("retry_count", 3)
    ///     .log();
    /// ```
    pub fn error_builder(&self) -> StructuredLogBuilder<'_> {
        StructuredLogBuilder::new(self, LogLevel::Error)
    }

    /// Create a fatal-level structured log builder
    ///
    /// # Example
    ///
    /// ```
    /// use rust_logger_system::Logger;
    ///
    /// let logger = Logger::new();
    /// logger.fatal_builder()
    ///     .message("System shutdown required")
    ///     .field("reason", "out_of_memory")
    ///     .log();
    /// ```
    pub fn fatal_builder(&self) -> StructuredLogBuilder<'_> {
        StructuredLogBuilder::new(self, LogLevel::Fatal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appenders::ConsoleAppender;

    #[test]
    fn test_structured_builder_basic() {
        let logger = Logger::builder()
            .min_level(LogLevel::Trace)
            .build();

        // Should not panic
        logger.info_builder()
            .message("Test message")
            .field("key", "value")
            .log();
    }

    #[test]
    fn test_structured_builder_multiple_fields() {
        let logger = Logger::builder()
            .min_level(LogLevel::Trace)
            .build();

        logger.debug_builder()
            .message("Multiple fields test")
            .field("string_field", "hello")
            .field("int_field", 42)
            .field("float_field", 3.14)
            .field("bool_field", true)
            .log();
    }

    #[test]
    fn test_structured_builder_all_levels() {
        let logger = Logger::builder()
            .min_level(LogLevel::Trace)
            .build();

        logger.trace_builder().message("Trace").log();
        logger.debug_builder().message("Debug").log();
        logger.info_builder().message("Info").log();
        logger.warn_builder().message("Warn").log();
        logger.error_builder().message("Error").log();
        logger.fatal_builder().message("Fatal").log();
    }

    #[test]
    fn test_structured_builder_with_context() {
        let logger = Logger::builder()
            .min_level(LogLevel::Trace)
            .build();

        let ctx = LogContext::new()
            .with_field("request_id", "abc-123")
            .with_field("user_id", 999);

        logger.info_builder()
            .message("With context")
            .fields(ctx)
            .field("additional", "field")
            .log();
    }

    #[test]
    fn test_structured_builder_empty_message() {
        let logger = Logger::builder()
            .min_level(LogLevel::Trace)
            .build();

        // Empty message should work
        logger.info_builder()
            .field("key", "value")
            .log();
    }
}
