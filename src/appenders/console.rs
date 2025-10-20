//! Console appender implementation

use crate::core::{Appender, LogEntry, LogLevel, Result};
use colored::Colorize;

pub struct ConsoleAppender {
    use_colors: bool,
}

impl ConsoleAppender {
    pub fn new() -> Self {
        Self { use_colors: true }
    }

    pub fn with_colors(use_colors: bool) -> Self {
        Self { use_colors }
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
            format!("{:5}", entry.level.to_str()).color(entry.level.color_code()).to_string()
        } else {
            format!("{:5}", entry.level.to_str())
        };

        let output = format!(
            "[{}] [{}] {} - {}",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
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
