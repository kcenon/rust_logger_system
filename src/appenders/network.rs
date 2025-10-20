//! Network appender for remote logging
//!
//! Sends log messages to a remote server over TCP.
//! Useful for centralized logging in distributed systems.

use crate::core::{Appender, LogEntry, LoggerError, Result};
use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Network appender that sends logs to a remote TCP server
///
/// # Example
///
/// ```no_run
/// use rust_logger_system::appenders::NetworkAppender;
/// use rust_logger_system::prelude::*;
///
/// let appender = NetworkAppender::new("127.0.0.1:8080")
///     .expect("Failed to connect to log server");
///
/// let mut logger = Logger::new();
/// logger.add_appender(Box::new(appender));
/// logger.info("This log will be sent to 127.0.0.1:8080");
/// ```
pub struct NetworkAppender {
    stream: Option<TcpStream>,
    address: String,
    reconnect_on_error: bool,
}

impl NetworkAppender {
    /// Create a new network appender
    ///
    /// # Arguments
    ///
    /// * `addr` - Socket address (e.g., "localhost:8080", "192.168.1.1:9000")
    ///
    /// # Errors
    ///
    /// Returns error if connection fails
    pub fn new(addr: impl ToSocketAddrs + ToString) -> Result<Self> {
        let address = addr.to_string();
        let stream = TcpStream::connect(&address)?;

        // Set timeouts to prevent hanging
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;

        // Enable TCP_NODELAY for low-latency logging
        stream.set_nodelay(true)?;

        Ok(Self {
            stream: Some(stream),
            address,
            reconnect_on_error: true,
        })
    }

    /// Enable or disable automatic reconnection on errors
    ///
    /// Default: enabled
    #[must_use]
    pub fn with_reconnect(mut self, enable: bool) -> Self {
        self.reconnect_on_error = enable;
        self
    }

    /// Attempt to reconnect to the server
    fn reconnect(&mut self) -> Result<()> {
        let stream = TcpStream::connect(&self.address)?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_nodelay(true)?;

        self.stream = Some(stream);
        Ok(())
    }
}

impl Appender for NetworkAppender {
    fn append(&mut self, entry: &LogEntry) -> Result<()> {
        // Format log entry
        let mut message = format!(
            "[{}] [{:5}] [{}] {}",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
            entry.level.to_str(),
            entry.thread_name.as_ref().unwrap_or(&entry.thread_id),
            entry.message
        );

        // Append context fields if present
        if let Some(ref context) = entry.context {
            message.push_str(" | ");
            message.push_str(&context.to_string());
        }

        message.push('\n');

        // Try to send log message
        let result = if let Some(ref mut stream) = self.stream {
            stream.write_all(message.as_bytes())
        } else {
            return Err(LoggerError::writer("Network stream not connected"));
        };

        // Handle errors with optional reconnection
        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                // Connection lost
                self.stream = None;

                if self.reconnect_on_error {
                    // Try to reconnect and resend
                    match self.reconnect() {
                        Ok(()) => {
                            // Resend the log message
                            if let Some(ref mut stream) = self.stream {
                                stream.write_all(message.as_bytes())?;
                            }
                            Ok(())
                        }
                        Err(reconnect_err) => {
                            // Reconnection failed, return original error
                            Err(LoggerError::writer(format!(
                                "Failed to send log and reconnect: {} (reconnect: {})",
                                e, reconnect_err
                            )))
                        }
                    }
                } else {
                    Err(e.into())
                }
            }
        }
    }

    fn flush(&mut self) -> Result<()> {
        if let Some(ref mut stream) = self.stream {
            stream.flush()?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "network"
    }
}

impl Drop for NetworkAppender {
    fn drop(&mut self) {
        // Ensure all buffered data is flushed
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::LogLevel;
    use chrono::Utc;

    #[test]
    fn test_network_appender_creation() {
        // This test will fail if no server is listening, which is expected
        let result = NetworkAppender::new("127.0.0.1:9999");

        // We expect connection to fail (no server running)
        assert!(result.is_err());
    }

    #[test]
    fn test_network_appender_with_reconnect() {
        if let Ok(appender) = NetworkAppender::new("127.0.0.1:9999") {
            let appender = appender.with_reconnect(false);
            assert!(!appender.reconnect_on_error);
        }
    }

    #[test]
    fn test_append_without_connection() {
        let mut appender = NetworkAppender {
            stream: None,
            address: "127.0.0.1:9999".to_string(),
            reconnect_on_error: false,
        };

        let entry = LogEntry {
            level: LogLevel::Info,
            message: "test".to_string(),
            timestamp: Utc::now(),
            file: Some("test.rs".to_string()),
            line: Some(42),
            module_path: Some("test".to_string()),
            thread_id: "main".to_string(),
            thread_name: Some("main".to_string()),
            context: None,
        };

        let result = appender.append(&entry);
        assert!(result.is_err());
    }
}
