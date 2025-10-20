//! Error types for the logger system

pub type Result<T> = std::result::Result<T, LoggerError>;

#[derive(Debug, thiserror::Error)]
pub enum LoggerError {
    /// IO error with context
    #[error("IO error while {operation}: {message}")]
    IoOperation {
        operation: String,
        message: String,
        #[source]
        source: std::io::Error,
    },

    /// Generic IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Queue full with buffer details
    #[error("Log queue full: {current}/{max} messages buffered")]
    QueueFull { current: usize, max: usize },

    /// Queue overflow with dropped message count
    #[error("Log queue overflow: dropped {dropped_count} messages")]
    QueueOverflow { dropped_count: usize },

    /// Logger already stopped
    #[error("Logger already stopped")]
    LoggerStopped,

    /// Invalid configuration with details
    #[error("Invalid configuration for {component}: {message}")]
    InvalidConfiguration { component: String, message: String },

    /// File appender error with path
    #[error("File appender error for '{path}': {message}")]
    FileAppenderError { path: String, message: String },

    /// File rotation error
    #[error("File rotation failed for '{path}': {message}")]
    FileRotationError { path: String, message: String },

    /// File lock error
    #[error("Failed to acquire file lock on '{path}'")]
    FileLockError { path: String },

    /// Writer error (generic)
    #[error("Writer error: {0}")]
    WriterError(String),

    /// Formatter error with format type
    #[error("Formatter error ({format_type}): {message}")]
    FormatterError {
        format_type: String,
        message: String,
    },

    /// Channel send error
    #[error("Failed to send log entry to async worker")]
    ChannelSendError,

    /// Channel receive error
    #[error("Failed to receive log entry from channel")]
    ChannelReceiveError,

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl LoggerError {
    /// Create an IO operation error with context
    pub fn io_operation(
        operation: impl Into<String>,
        message: impl Into<String>,
        source: std::io::Error,
    ) -> Self {
        LoggerError::IoOperation {
            operation: operation.into(),
            message: message.into(),
            source,
        }
    }

    /// Create a queue full error with buffer details
    pub fn queue_full(current: usize, max: usize) -> Self {
        LoggerError::QueueFull { current, max }
    }

    /// Create a queue overflow error
    pub fn queue_overflow(dropped_count: usize) -> Self {
        LoggerError::QueueOverflow { dropped_count }
    }

    /// Create an invalid configuration error
    pub fn config(component: impl Into<String>, message: impl Into<String>) -> Self {
        LoggerError::InvalidConfiguration {
            component: component.into(),
            message: message.into(),
        }
    }

    /// Create a file appender error
    pub fn file_appender(path: impl Into<String>, message: impl Into<String>) -> Self {
        LoggerError::FileAppenderError {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a file rotation error
    pub fn file_rotation(path: impl Into<String>, message: impl Into<String>) -> Self {
        LoggerError::FileRotationError {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a file lock error
    pub fn file_lock(path: impl Into<String>) -> Self {
        LoggerError::FileLockError {
            path: path.into(),
        }
    }

    /// Create a formatter error
    pub fn formatter(format_type: impl Into<String>, message: impl Into<String>) -> Self {
        LoggerError::FormatterError {
            format_type: format_type.into(),
            message: message.into(),
        }
    }

    /// Create a writer error (generic)
    pub fn writer<S: Into<String>>(msg: S) -> Self {
        LoggerError::WriterError(msg.into())
    }

    /// Create a generic error
    pub fn other<S: Into<String>>(msg: S) -> Self {
        LoggerError::Other(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = LoggerError::queue_full(100, 1000);
        assert!(matches!(err, LoggerError::QueueFull { .. }));

        let err = LoggerError::config("FileAppender", "Invalid path");
        assert!(matches!(err, LoggerError::InvalidConfiguration { .. }));

        let err = LoggerError::file_appender("/var/log/app.log", "Permission denied");
        assert!(matches!(err, LoggerError::FileAppenderError { .. }));
    }

    #[test]
    fn test_error_display() {
        let err = LoggerError::queue_full(100, 1000);
        assert_eq!(
            err.to_string(),
            "Log queue full: 100/1000 messages buffered"
        );

        let err = LoggerError::file_rotation("/var/log/app.log", "Disk full");
        assert_eq!(
            err.to_string(),
            "File rotation failed for '/var/log/app.log': Disk full"
        );

        let err = LoggerError::formatter("JSON", "Invalid field type");
        assert_eq!(
            err.to_string(),
            "Formatter error (JSON): Invalid field type"
        );
    }

    #[test]
    fn test_io_operation_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = LoggerError::io_operation("writing log file", "cannot write to file", io_err);

        assert!(matches!(err, LoggerError::IoOperation { .. }));
        assert!(err.to_string().contains("writing log file"));
        assert!(err.to_string().contains("cannot write to file"));
    }
}
