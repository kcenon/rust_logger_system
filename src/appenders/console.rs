//! Console appender implementation

use crate::core::{Appender, LogEntry, LogLevel, Result, TimestampFormat};
use colored::Colorize;

pub struct ConsoleAppender {
    use_colors: bool,
    timestamp_format: TimestampFormat,
}

impl ConsoleAppender {
    pub fn new() -> Self {
        Self {
            use_colors: true,
            timestamp_format: TimestampFormat::default(),
        }
    }

    pub fn with_colors(use_colors: bool) -> Self {
        Self {
            use_colors,
            timestamp_format: TimestampFormat::default(),
        }
    }

    /// Set the timestamp format for this appender
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_logger_system::appenders::ConsoleAppender;
    /// use rust_logger_system::TimestampFormat;
    ///
    /// let appender = ConsoleAppender::new()
    ///     .with_timestamp_format(TimestampFormat::Iso8601Micros);
    /// ```
    #[must_use]
    pub fn with_timestamp_format(mut self, format: TimestampFormat) -> Self {
        self.timestamp_format = format;
        self
    }

    /// Set a custom timestamp format using a strftime-compatible format string
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_logger_system::appenders::ConsoleAppender;
    ///
    /// let appender = ConsoleAppender::new()
    ///     .with_custom_timestamp("%d/%b/%Y:%H:%M:%S %z");
    /// ```
    #[must_use]
    pub fn with_custom_timestamp(mut self, format_str: &str) -> Self {
        self.timestamp_format = TimestampFormat::Custom(format_str.to_string());
        self
    }
}

impl Default for ConsoleAppender {
    fn default() -> Self {
        Self::new()
    }
}

impl Appender for ConsoleAppender {
    fn append(&mut self, entry: &LogEntry) -> Result<()> {
        let level_str = if self.use_colors {
            format!("{:5}", entry.level.to_str())
                .color(entry.level.color_code())
                .to_string()
        } else {
            format!("{:5}", entry.level.to_str())
        };

        let timestamp_str = self.timestamp_format.format(&entry.timestamp);

        let output = format!(
            "[{}] [{}] {} - {}",
            timestamp_str,
            level_str,
            entry.thread_name.as_ref().unwrap_or(&entry.thread_id),
            entry.message
        );

        // Route Error and Fatal levels to stderr, others to stdout
        match entry.level {
            LogLevel::Error | LogLevel::Fatal => eprintln!("{}", output),
            _ => println!("{}", output),
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        use std::io::Write;
        // Flush both stdout and stderr since we write to both
        std::io::stdout().flush()?;
        std::io::stderr().flush()?;
        Ok(())
    }

    fn name(&self) -> &str {
        "console"
    }
}
