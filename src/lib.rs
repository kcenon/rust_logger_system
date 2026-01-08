//! # Rust Logger System
//!
//! A production-ready, high-performance Rust logging framework with asynchronous
//! processing and multiple output targets.
//!
//! ## Features
//!
//! - **High Performance**: Asynchronous logging with minimal overhead
//! - **Multiple Appenders**: Console, file, and custom appenders
//! - **Thread Safe**: Designed for concurrent environments
//! - **Easy to Use**: Simple and intuitive API

pub mod appenders;
pub mod core;
pub mod macros;

pub mod prelude {
    pub use crate::appenders::{ConsoleAppender, FileAppender};
    pub use crate::core::{
        Appender, FormatterConfig, LogEntry, LogLevel, Logger, LoggerBuilder, LoggerError,
        LoggerMetrics, LogPriority, OverflowCallback, OverflowPolicy, PriorityConfig, Result,
        TimestampFormat, DEFAULT_SHUTDOWN_TIMEOUT,
    };
}

pub use appenders::{ConsoleAppender, FileAppender};
pub use core::{
    Appender, FormatterConfig, LogEntry, LogLevel, Logger, LoggerBuilder, LoggerError,
    LoggerMetrics, LogPriority, OverflowCallback, OverflowPolicy, PriorityConfig, Result,
    TimestampFormat, DEFAULT_SHUTDOWN_TIMEOUT,
};
