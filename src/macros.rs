//! Logging macros for ergonomic log message formatting.
//!
//! These macros provide a convenient interface for logging with automatic
//! string formatting, similar to `println!` and `format!`.
//!
//! # Examples
//!
//! ```
//! use rust_logger_system::prelude::*;
//! use rust_logger_system::info;
//!
//! let logger = Logger::new();
//!
//! // Basic logging
//! info!(logger, "Server started");
//!
//! // With format arguments
//! let port = 8080;
//! info!(logger, "Server listening on port {}", port);
//!
//! // Complex formatting
//! let user_id = 42;
//! let action = "login";
//! info!(logger, "User {} performed action: {}", user_id, action);
//! ```

/// Log a message with automatic formatting.
///
/// # Examples
///
/// ```
/// # use rust_logger_system::prelude::*;
/// # let logger = Logger::new();
/// use rust_logger_system::log;
/// log!(logger, LogLevel::Info, "Simple message");
/// log!(logger, LogLevel::Error, "Error code: {}", 500);
/// ```
#[macro_export]
macro_rules! log {
    ($logger:expr, $level:expr, $($arg:tt)+) => {
        $logger.log($level, format!($($arg)+))
    };
}

/// Log a trace-level message.
///
/// # Examples
///
/// ```
/// # use rust_logger_system::prelude::*;
/// # let mut logger = Logger::new();
/// # logger.set_min_level(LogLevel::Trace);
/// use rust_logger_system::trace;
/// trace!(logger, "Entering function: calculate()");
/// trace!(logger, "Variable value: {}", 42);
/// ```
#[macro_export]
macro_rules! trace {
    ($logger:expr, $($arg:tt)+) => {
        $crate::log!($logger, $crate::LogLevel::Trace, $($arg)+)
    };
}

/// Log a debug-level message.
///
/// # Examples
///
/// ```
/// # use rust_logger_system::prelude::*;
/// # let logger = Logger::new();
/// use rust_logger_system::debug;
/// debug!(logger, "Debug information");
/// debug!(logger, "Counter value: {}", 10);
/// ```
#[macro_export]
macro_rules! debug {
    ($logger:expr, $($arg:tt)+) => {
        $crate::log!($logger, $crate::LogLevel::Debug, $($arg)+)
    };
}

/// Log an info-level message.
///
/// # Examples
///
/// ```
/// # use rust_logger_system::prelude::*;
/// # let logger = Logger::new();
/// use rust_logger_system::info;
/// info!(logger, "Application started");
/// info!(logger, "Processing {} items", 100);
/// ```
#[macro_export]
macro_rules! info {
    ($logger:expr, $($arg:tt)+) => {
        $crate::log!($logger, $crate::LogLevel::Info, $($arg)+)
    };
}

/// Log a warning-level message.
///
/// # Examples
///
/// ```
/// # use rust_logger_system::prelude::*;
/// # let logger = Logger::new();
/// use rust_logger_system::warn;
/// warn!(logger, "Low disk space");
/// warn!(logger, "Retry attempt {} of {}", 3, 5);
/// ```
#[macro_export]
macro_rules! warn {
    ($logger:expr, $($arg:tt)+) => {
        $crate::log!($logger, $crate::LogLevel::Warn, $($arg)+)
    };
}

/// Log an error-level message.
///
/// # Examples
///
/// ```
/// # use rust_logger_system::prelude::*;
/// # let logger = Logger::new();
/// use rust_logger_system::error;
/// error!(logger, "Failed to connect to database");
/// error!(logger, "Error code: {}, message: {}", 500, "Internal error");
/// ```
#[macro_export]
macro_rules! error {
    ($logger:expr, $($arg:tt)+) => {
        $crate::log!($logger, $crate::LogLevel::Error, $($arg)+)
    };
}

/// Log a fatal-level message.
///
/// # Examples
///
/// ```
/// # use rust_logger_system::prelude::*;
/// # let logger = Logger::new();
/// use rust_logger_system::fatal;
/// fatal!(logger, "Critical system failure");
/// fatal!(logger, "Unable to recover from error: {}", "disk full");
/// ```
#[macro_export]
macro_rules! fatal {
    ($logger:expr, $($arg:tt)+) => {
        $crate::log!($logger, $crate::LogLevel::Fatal, $($arg)+)
    };
}

#[cfg(test)]
mod tests {
    use crate::core::{Logger, LogLevel};

    #[test]
    fn test_log_macro() {
        let logger = Logger::new();
        log!(logger, LogLevel::Info, "Test message");
        log!(logger, LogLevel::Info, "Formatted: {}", 42);
    }

    #[test]
    fn test_trace_macro() {
        let mut logger = Logger::new();
        logger.set_min_level(LogLevel::Trace);
        trace!(logger, "Trace message");
        trace!(logger, "Value: {}", 10);
    }

    #[test]
    fn test_debug_macro() {
        let logger = Logger::new();
        debug!(logger, "Debug message");
        debug!(logger, "Count: {}", 5);
    }

    #[test]
    fn test_info_macro() {
        let logger = Logger::new();
        info!(logger, "Info message");
        info!(logger, "Items: {}", 100);
    }

    #[test]
    fn test_warn_macro() {
        let logger = Logger::new();
        warn!(logger, "Warning message");
        warn!(logger, "Retry {} of {}", 1, 3);
    }

    #[test]
    fn test_error_macro() {
        let logger = Logger::new();
        error!(logger, "Error message");
        error!(logger, "Code: {}", 500);
    }

    #[test]
    fn test_fatal_macro() {
        let logger = Logger::new();
        fatal!(logger, "Fatal message");
        fatal!(logger, "Critical failure: {}", "system");
    }
}
