//! Main logger implementation

use super::{
    appender::Appender, error::Result, log_context::LogContext, log_entry::LogEntry,
    log_level::LogLevel,
};
use crossbeam_channel::{bounded, Sender};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Default shutdown timeout for logger cleanup (5 seconds)
///
/// This timeout is used when the logger is dropped without explicit shutdown.
/// For custom timeout control, use the `shutdown()` method instead.
pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Logger {
    min_level: Arc<RwLock<LogLevel>>,
    appenders: Arc<RwLock<Vec<Box<dyn Appender>>>>,
    sender: Option<Sender<LogEntry>>,
    async_handle: Option<thread::JoinHandle<()>>,
    /// Counter for failed log attempts (for observability)
    failed_writes: Arc<AtomicU64>,
    /// Counter for sync fallback events when async buffer is full (for observability)
    sync_fallbacks: Arc<AtomicU64>,
}

impl Logger {
    #[must_use]
    pub fn new() -> Self {
        Self {
            min_level: Arc::new(RwLock::new(LogLevel::Info)),
            appenders: Arc::new(RwLock::new(Vec::new())),
            sender: None,
            async_handle: None,
            failed_writes: Arc::new(AtomicU64::new(0)),
            sync_fallbacks: Arc::new(AtomicU64::new(0)),
        }
    }

    #[must_use]
    pub fn with_async(buffer_size: usize) -> Self {
        let (sender, receiver) = bounded(buffer_size);
        let appenders: Arc<RwLock<Vec<Box<dyn Appender>>>> = Arc::new(RwLock::new(Vec::new()));
        let appenders_clone = Arc::clone(&appenders);
        let failed_writes = Arc::new(AtomicU64::new(0));
        let failed_writes_clone = Arc::clone(&failed_writes);

        let handle = thread::spawn(move || {
            // Batch processing: collect multiple entries before writing
            // This improves performance by reducing lock contention and I/O operations
            const BATCH_SIZE: usize = 50;
            const BATCH_TIMEOUT_MS: u64 = 10;

            let mut batch = Vec::with_capacity(BATCH_SIZE);

            loop {
                // Try to receive first entry (blocking)
                match receiver.recv() {
                    Ok(entry) => batch.push(entry),
                    Err(_) => {
                        // Channel closed, flush remaining batch and exit
                        if !batch.is_empty() {
                            Self::process_batch(&appenders_clone, &batch, &failed_writes_clone);
                        }
                        break;
                    }
                }

                // Try to collect more entries without blocking (up to BATCH_SIZE)
                while batch.len() < BATCH_SIZE {
                    match receiver.try_recv() {
                        Ok(entry) => batch.push(entry),
                        Err(_) => break, // No more entries available immediately
                    }
                }

                // Process batch when full or after timeout
                if batch.len() >= BATCH_SIZE {
                    Self::process_batch(&appenders_clone, &batch, &failed_writes_clone);
                    batch.clear();
                } else if !batch.is_empty() {
                    // Small batch - wait a bit for more entries
                    thread::sleep(std::time::Duration::from_millis(BATCH_TIMEOUT_MS));

                    // Try one more time to collect entries
                    while batch.len() < BATCH_SIZE {
                        match receiver.try_recv() {
                            Ok(entry) => batch.push(entry),
                            Err(_) => break,
                        }
                    }

                    // Process whatever we have
                    Self::process_batch(&appenders_clone, &batch, &failed_writes_clone);
                    batch.clear();
                }
            }
        });

        Self {
            min_level: Arc::new(RwLock::new(LogLevel::Info)),
            appenders,
            sender: Some(sender),
            async_handle: Some(handle),
            failed_writes,
            sync_fallbacks: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Process a batch of log entries
    ///
    /// Helper method for batch processing in async logger thread
    ///
    /// **Per-Appender Panic Isolation**: Each appender is wrapped in catch_unwind
    /// to prevent a single failing appender from disrupting the entire logger.
    /// If one appender panics, other appenders will continue to receive log entries.
    fn process_batch(
        appenders: &Arc<RwLock<Vec<Box<dyn Appender>>>>,
        batch: &[LogEntry],
        failed_writes: &Arc<AtomicU64>,
    ) {
        let mut appenders_guard = appenders.write();
        let mut total_errors = 0;

        // Process each entry in the batch
        for entry in batch {
            let mut has_error = false;

            // Per-appender panic isolation: wrap each appender call separately
            for (idx, appender) in appenders_guard.iter_mut().enumerate() {
                let append_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    appender.append(entry)
                }));

                match append_result {
                    Ok(Ok(())) => {
                        // Success - appender handled the log entry
                    }
                    Ok(Err(e)) => {
                        // Appender returned an error (not a panic)
                        eprintln!("[LOGGER ERROR] Appender #{} failed: {}", idx, e);
                        has_error = true;
                    }
                    Err(panic_info) => {
                        // Appender panicked - extract panic message
                        let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                            s.to_string()
                        } else if let Some(s) = panic_info.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            "Unknown panic".to_string()
                        };
                        eprintln!(
                            "[LOGGER CRITICAL] Appender #{} panicked: {}. \
                             Other appenders continue to function.",
                            idx, panic_msg
                        );
                        has_error = true;
                    }
                }
            }

            if has_error {
                total_errors += 1;
            }
        }

        if total_errors > 0 {
            failed_writes.fetch_add(total_errors, Ordering::Relaxed);
        }

        // Flush after each batch to ensure timely writes
        // Also use per-appender panic isolation for flush operations
        for (idx, appender) in appenders_guard.iter_mut().enumerate() {
            let flush_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                appender.flush()
            }));

            match flush_result {
                Ok(Ok(())) => {
                    // Flush succeeded
                }
                Ok(Err(e)) => {
                    eprintln!("[LOGGER ERROR] Appender #{} flush failed: {}", idx, e);
                }
                Err(panic_info) => {
                    let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_info.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "Unknown panic".to_string()
                    };
                    eprintln!(
                        "[LOGGER CRITICAL] Appender #{} panicked during flush: {}. \
                         Other appenders continue to function.",
                        idx, panic_msg
                    );
                }
            }
        }
    }

    /// Process log entry synchronously with per-appender panic isolation
    ///
    /// This helper ensures that even in synchronous logging, one failing appender
    /// doesn't prevent other appenders from receiving log entries.
    fn process_sync(
        appenders: &mut Vec<Box<dyn Appender>>,
        entry: &LogEntry,
        failed_writes: &Arc<AtomicU64>,
    ) -> bool {
        let mut has_error = false;

        for (idx, appender) in appenders.iter_mut().enumerate() {
            let append_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                appender.append(entry)
            }));

            match append_result {
                Ok(Ok(())) => {
                    // Success
                }
                Ok(Err(e)) => {
                    eprintln!("[LOGGER ERROR] Appender #{} failed (sync): {}", idx, e);
                    has_error = true;
                }
                Err(panic_info) => {
                    let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_info.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "Unknown panic".to_string()
                    };
                    eprintln!(
                        "[LOGGER CRITICAL] Appender #{} panicked (sync): {}. \
                         Other appenders continue to function.",
                        idx, panic_msg
                    );
                    has_error = true;
                }
            }
        }

        if has_error {
            failed_writes.fetch_add(1, Ordering::Relaxed);
        }

        has_error
    }

    pub fn add_appender(&mut self, appender: Box<dyn Appender>) {
        let mut appenders = self.appenders.write();
        appenders.push(appender);
    }

    pub fn set_min_level(&mut self, level: LogLevel) {
        let mut min_level = self.min_level.write();
        *min_level = level;
    }

    pub fn log(&self, level: LogLevel, message: impl Into<String>) {
        if level < *self.min_level.read() {
            return;
        }

        let entry = LogEntry::new(level, message.into());

        if let Some(ref sender) = self.sender {
            // Handle backpressure: fall back to synchronous logging if buffer is full
            match sender.try_send(entry) {
                Ok(_) => {}
                Err(crossbeam_channel::TrySendError::Full(entry)) => {
                    // Buffer full - log synchronously to avoid dropping critical messages
                    // Increment fallback counter for observability
                    self.sync_fallbacks.fetch_add(1, Ordering::Relaxed);

                    // Use try_write to prevent deadlock if async worker holds the lock
                    if let Some(mut appenders) = self.appenders.try_write() {
                        eprintln!(
                            "[LOGGER WARNING] Async buffer full (fallback #{}). Logging synchronously. \
                             Consider increasing buffer size or reducing log volume.",
                            self.sync_fallbacks.load(Ordering::Relaxed)
                        );
                        Self::process_sync(&mut appenders, &entry, &self.failed_writes);
                    } else {
                        // Lock unavailable - drop log to prevent deadlock
                        eprintln!(
                            "[LOGGER WARNING] Buffer full and appenders lock unavailable. \
                             Dropping log entry to prevent deadlock. Message: {:?}",
                            entry.message
                        );
                        self.failed_writes.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                    // Logger is shutting down, silently ignore
                }
            }
        } else {
            let mut appenders = self.appenders.write();
            Self::process_sync(&mut appenders, &entry, &self.failed_writes);
        }
    }

    /// Get the number of failed log write attempts
    ///
    /// This counter tracks log entries that failed to write to any appender.
    /// Useful for monitoring logger health.
    pub fn failed_write_count(&self) -> u64 {
        self.failed_writes.load(Ordering::Relaxed)
    }

    /// Get the number of synchronous fallback events
    ///
    /// This counter tracks how many times the async logger fell back to
    /// synchronous logging due to a full buffer. Each fallback indicates
    /// backpressure in the logging system.
    ///
    /// **High fallback counts indicate:**
    /// - The async buffer is too small for the log volume
    /// - Appenders are slow and can't keep up with log generation
    /// - Potential performance impact from blocking on sync writes
    ///
    /// **Recommended actions:**
    /// - Increase the buffer size in `Logger::with_async()`
    /// - Optimize or reduce log volume
    /// - Check appender performance (file I/O, network, etc.)
    ///
    /// # Example
    ///
    /// ```
    /// use rust_logger_system::Logger;
    ///
    /// let logger = Logger::with_async(100);
    ///
    /// // After logging operations...
    /// let fallbacks = logger.sync_fallback_count();
    /// if fallbacks > 0 {
    ///     eprintln!("Warning: {} sync fallbacks detected", fallbacks);
    /// }
    /// ```
    pub fn sync_fallback_count(&self) -> u64 {
        self.sync_fallbacks.load(Ordering::Relaxed)
    }

    pub fn flush(&self) -> Result<()> {
        let mut appenders = self.appenders.write();
        for appender in appenders.iter_mut() {
            appender.flush()?;
        }
        Ok(())
    }

    #[inline]
    pub fn trace(&self, message: impl Into<String>) {
        self.log(LogLevel::Trace, message);
    }

    #[inline]
    pub fn debug(&self, message: impl Into<String>) {
        self.log(LogLevel::Debug, message);
    }

    #[inline]
    pub fn info(&self, message: impl Into<String>) {
        self.log(LogLevel::Info, message);
    }

    #[inline]
    pub fn warn(&self, message: impl Into<String>) {
        self.log(LogLevel::Warn, message);
    }

    #[inline]
    pub fn error(&self, message: impl Into<String>) {
        self.log(LogLevel::Error, message);
    }

    #[inline]
    pub fn fatal(&self, message: impl Into<String>) {
        self.log(LogLevel::Fatal, message);
    }

    /// Log with structured context fields
    pub fn log_with_context(
        &self,
        level: LogLevel,
        message: impl Into<String>,
        context: LogContext,
    ) {
        if level < *self.min_level.read() {
            return;
        }

        let entry = LogEntry::new(level, message.into()).with_context(context);

        if let Some(ref sender) = self.sender {
            match sender.try_send(entry) {
                Ok(_) => {}
                Err(crossbeam_channel::TrySendError::Full(entry)) => {
                    // Buffer full - log synchronously to avoid dropping critical messages
                    // Increment fallback counter for observability
                    self.sync_fallbacks.fetch_add(1, Ordering::Relaxed);

                    // Use try_write to prevent deadlock if async worker holds the lock
                    if let Some(mut appenders) = self.appenders.try_write() {
                        eprintln!(
                            "[LOGGER WARNING] Async buffer full (fallback #{}). Logging synchronously. \
                             Consider increasing buffer size or reducing log volume.",
                            self.sync_fallbacks.load(Ordering::Relaxed)
                        );
                        Self::process_sync(&mut appenders, &entry, &self.failed_writes);
                    } else {
                        // Lock unavailable - drop log to prevent deadlock
                        eprintln!(
                            "[LOGGER WARNING] Buffer full and appenders lock unavailable. \
                             Dropping log entry to prevent deadlock. Message: {:?}",
                            entry.message
                        );
                        self.failed_writes.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {}
            }
        } else {
            let mut appenders = self.appenders.write();
            Self::process_sync(&mut appenders, &entry, &self.failed_writes);
        }
    }

    /// Helper for structured info logging
    pub fn info_with_context(&self, message: impl Into<String>, context: LogContext) {
        self.log_with_context(LogLevel::Info, message, context);
    }

    /// Helper for structured error logging
    pub fn error_with_context(&self, message: impl Into<String>, context: LogContext) {
        self.log_with_context(LogLevel::Error, message, context);
    }

    /// Gracefully shutdown the logger with a custom timeout
    ///
    /// This method ensures all pending log entries are written before shutdown.
    /// It's useful when you need explicit control over logger shutdown timing.
    ///
    /// **Note**: When the logger is dropped without calling `shutdown()` explicitly,
    /// it uses [`DEFAULT_SHUTDOWN_TIMEOUT`] (5 seconds). Use this method if you need
    /// a different timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for pending logs to drain
    ///
    /// # Returns
    ///
    /// `true` if shutdown completed successfully within timeout, `false` otherwise
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_logger_system::{Logger, DEFAULT_SHUTDOWN_TIMEOUT};
    /// use std::time::Duration;
    ///
    /// let mut logger = Logger::with_async(1000);
    /// logger.info("Important message");
    ///
    /// // Use custom timeout (longer than default)
    /// if !logger.shutdown(Duration::from_secs(10)) {
    ///     eprintln!("Warning: Logger shutdown timed out");
    /// }
    ///
    /// // Or use the default timeout explicitly
    /// // logger.shutdown(DEFAULT_SHUTDOWN_TIMEOUT);
    /// ```
    pub fn shutdown(&mut self, timeout: Duration) -> bool {
        // Close the channel to signal worker thread
        drop(self.sender.take());

        // Wait for async worker to finish draining all messages
        if let Some(handle) = self.async_handle.take() {
            let start = std::time::Instant::now();

            loop {
                if handle.is_finished() {
                    // Thread finished, join it to check for panics
                    if let Err(e) = handle.join() {
                        eprintln!("[LOGGER ERROR] Async worker thread panicked during shutdown: {:?}", e);
                        return false;
                    }
                    break;
                }

                if start.elapsed() >= timeout {
                    eprintln!(
                        "[LOGGER WARNING] Async worker thread did not finish within timeout. \
                         Some logs may be lost."
                    );
                    return false;
                }

                // Small sleep to avoid busy-waiting
                thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        // Final flush
        if let Err(e) = self.flush() {
            eprintln!("[LOGGER ERROR] Failed to flush during shutdown: {}", e);
            return false;
        }

        true
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Logger {
    fn drop(&mut self) {
        // Close the channel first to signal worker thread to finish
        // This allows the worker to drain all pending messages before exiting
        drop(self.sender.take());

        // Wait for async worker to finish draining all messages
        if let Some(handle) = self.async_handle.take() {
            // Use a timeout to prevent hanging indefinitely
            let start = std::time::Instant::now();
            let timeout = DEFAULT_SHUTDOWN_TIMEOUT;

            // Attempt to join the thread with timeout
            loop {
                if handle.is_finished() {
                    // Thread finished, join it to check for panics
                    if let Err(e) = handle.join() {
                        eprintln!("[LOGGER ERROR] Async worker thread panicked during shutdown: {:?}", e);
                    }
                    break;
                }

                if start.elapsed() >= timeout {
                    eprintln!(
                        "[LOGGER WARNING] Async worker thread did not finish within {:?} timeout. \
                         Some logs may be lost.",
                        timeout
                    );
                    break;
                }

                // Small sleep to avoid busy-waiting
                thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        // Final flush of any synchronous appenders
        if let Err(e) = self.flush() {
            eprintln!("[LOGGER ERROR] Failed to flush during shutdown: {}", e);
        }

        // Report any failed writes
        let failed = self.failed_writes.load(Ordering::Relaxed);
        if failed > 0 {
            eprintln!("[LOGGER WARNING] Logger shutting down with {} failed writes", failed);
        }
    }
}

/// Builder for constructing Logger with a fluent API
///
/// # Example
/// ```
/// use rust_logger_system::prelude::*;
///
/// let logger = Logger::builder()
///     .min_level(LogLevel::Debug)
///     .appender(ConsoleAppender::new())
///     .async_mode(1000)
///     .build();
/// ```
pub struct LoggerBuilder {
    min_level: LogLevel,
    appenders: Vec<Box<dyn Appender>>,
    async_buffer: Option<usize>,
}

impl LoggerBuilder {
    /// Create a new builder with default values
    pub fn new() -> Self {
        Self {
            min_level: LogLevel::Info,
            appenders: Vec::new(),
            async_buffer: None,
        }
    }

    /// Set minimum log level
    #[must_use = "builder methods return a new value"]
    pub fn min_level(mut self, level: LogLevel) -> Self {
        self.min_level = level;
        self
    }

    /// Add an appender
    #[must_use = "builder methods return a new value"]
    pub fn appender<A: Appender + 'static>(mut self, appender: A) -> Self {
        self.appenders.push(Box::new(appender));
        self
    }

    /// Enable async mode with specified buffer size
    ///
    /// If not called, the logger will use synchronous mode.
    #[must_use = "builder methods return a new value"]
    pub fn async_mode(mut self, buffer_size: usize) -> Self {
        self.async_buffer = Some(buffer_size);
        self
    }

    /// Build the Logger
    pub fn build(self) -> Logger {
        let mut logger = if let Some(size) = self.async_buffer {
            Logger::with_async(size)
        } else {
            Logger::new()
        };

        logger.set_min_level(self.min_level);
        for appender in self.appenders {
            logger.add_appender(appender);
        }

        logger
    }
}

impl Default for LoggerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Logger {
    /// Create a builder for Logger
    ///
    /// # Example
    /// ```
    /// use rust_logger_system::prelude::*;
    ///
    /// let logger = Logger::builder()
    ///     .min_level(LogLevel::Debug)
    ///     .async_mode(1000)
    ///     .build();
    /// ```
    #[must_use]
    pub fn builder() -> LoggerBuilder {
        LoggerBuilder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appenders::ConsoleAppender;

    #[test]
    fn test_builder_basic() {
        let logger = Logger::builder()
            .min_level(LogLevel::Debug)
            .build();

        // Verify the logger was created
        assert_eq!(logger.failed_write_count(), 0);
    }

    #[test]
    fn test_builder_with_appender() {
        let logger = Logger::builder()
            .min_level(LogLevel::Info)
            .appender(ConsoleAppender::new())
            .build();

        // Verify logger was created with appender
        assert_eq!(logger.failed_write_count(), 0);
    }

    #[test]
    fn test_builder_async_mode() {
        let logger = Logger::builder()
            .min_level(LogLevel::Trace)
            .async_mode(1000)
            .build();

        // Verify async logger was created
        assert_eq!(logger.sync_fallback_count(), 0);
        assert_eq!(logger.failed_write_count(), 0);
    }

    #[test]
    fn test_builder_full_configuration() {
        let logger = Logger::builder()
            .min_level(LogLevel::Debug)
            .appender(ConsoleAppender::new())
            .async_mode(500)
            .build();

        // Log a message to verify it works
        logger.info("Test message");
        assert_eq!(logger.failed_write_count(), 0);
    }

    #[test]
    fn test_builder_default() {
        let builder = LoggerBuilder::default();
        let logger = builder.build();

        // Default logger should have Info level
        assert_eq!(logger.failed_write_count(), 0);
    }
}
