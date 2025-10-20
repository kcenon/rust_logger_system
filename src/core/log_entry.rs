//! Log entry structure

use super::log_context::LogContext;
use super::log_level::LogLevel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

// Thread-local caches for thread information to avoid repeated allocations
thread_local! {
    static THREAD_ID_CACHE: RefCell<Option<String>> = const { RefCell::new(None) };
    static THREAD_NAME_CACHE: RefCell<Option<Option<String>>> = const { RefCell::new(None) };
}

/// Get cached thread ID, computing and caching it on first access
fn get_thread_id() -> String {
    THREAD_ID_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.is_none() {
            *cache = Some(format!("{:?}", std::thread::current().id()));
        }
        cache.as_ref().expect("thread_id cache initialized in previous line").clone()
    })
}

/// Get cached thread name, computing and caching it on first access
fn get_thread_name() -> Option<String> {
    THREAD_NAME_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.is_none() {
            *cache = Some(std::thread::current().name().map(String::from));
        }
        cache.as_ref().expect("thread_name cache initialized in previous line").clone()
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub module_path: Option<String>,
    pub thread_id: String,
    pub thread_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<LogContext>,
}

impl LogEntry {
    /// Sanitize log message to prevent log injection attacks
    ///
    /// Replaces newlines, carriage returns, and tabs with escape sequences
    /// to prevent attackers from injecting fake log entries.
    fn sanitize_message(message: &str) -> String {
        message
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }

    pub fn new(level: LogLevel, message: String) -> Self {
        Self {
            level,
            message: Self::sanitize_message(&message),
            timestamp: Utc::now(),
            file: None,
            line: None,
            module_path: None,
            thread_id: get_thread_id(),
            thread_name: get_thread_name(),
            context: None,
        }
    }

    pub fn with_location(mut self, file: &str, line: u32, module_path: &str) -> Self {
        self.file = Some(file.to_string());
        self.line = Some(line);
        self.module_path = Some(module_path.to_string());
        self
    }

    pub fn with_context(mut self, context: LogContext) -> Self {
        self.context = Some(context);
        self
    }
}
