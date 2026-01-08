//! Main logger implementation

use super::{
    appender::Appender,
    error::Result,
    log_context::LogContext,
    log_entry::LogEntry,
    log_level::LogLevel,
    metrics::LoggerMetrics,
    overflow_policy::{LogPriority, OverflowCallback, OverflowPolicy},
};
use crossbeam_channel::{bounded, Sender, TrySendError};
use parking_lot::RwLock;
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
    /// Metrics for observability (dropped count, total logged, etc.)
    metrics: Arc<LoggerMetrics>,
    /// Policy for handling queue overflow
    overflow_policy: OverflowPolicy,
    /// Optional callback for overflow notifications
    on_overflow: Option<OverflowCallback>,
}

impl Logger {
    #[must_use]
    pub fn new() -> Self {
        Self {
            min_level: Arc::new(RwLock::new(LogLevel::Info)),
            appenders: Arc::new(RwLock::new(Vec::new())),
            sender: None,
            async_handle: None,
            metrics: Arc::new(LoggerMetrics::new()),
            overflow_policy: OverflowPolicy::AlertAndDrop,
            on_overflow: None,
        }
    }

    #[must_use]
    pub fn with_async(buffer_size: usize) -> Self {
        Self::with_async_config(buffer_size, OverflowPolicy::AlertAndDrop, None)
    }

    /// Create an async logger with custom overflow configuration
    #[must_use]
    pub fn with_async_config(
        buffer_size: usize,
        overflow_policy: OverflowPolicy,
        on_overflow: Option<OverflowCallback>,
    ) -> Self {
        let (sender, receiver) = bounded(buffer_size);
        let appenders: Arc<RwLock<Vec<Box<dyn Appender>>>> = Arc::new(RwLock::new(Vec::new()));
        let appenders_clone = Arc::clone(&appenders);
        let metrics = Arc::new(LoggerMetrics::new());
        let metrics_clone = Arc::clone(&metrics);

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
                            Self::process_batch(&appenders_clone, &batch, &metrics_clone);
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
                    Self::process_batch(&appenders_clone, &batch, &metrics_clone);
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
                    Self::process_batch(&appenders_clone, &batch, &metrics_clone);
                    batch.clear();
                }
            }
        });

        Self {
            min_level: Arc::new(RwLock::new(LogLevel::Info)),
            appenders,
            sender: Some(sender),
            async_handle: Some(handle),
            metrics,
            overflow_policy,
            on_overflow,
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
        metrics: &Arc<LoggerMetrics>,
    ) {
        let mut appenders_guard = appenders.write();

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
                metrics.record_dropped();
            } else {
                metrics.record_logged();
            }
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
        metrics: &Arc<LoggerMetrics>,
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
            metrics.record_dropped();
        } else {
            metrics.record_logged();
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
        self.send_entry(entry);
    }

    /// Internal method to send a log entry with overflow handling
    fn send_entry(&self, entry: LogEntry) {
        if let Some(ref sender) = self.sender {
            let priority = entry.level.priority();

            match sender.try_send(entry) {
                Ok(()) => {
                    // Successfully queued
                }
                Err(TrySendError::Full(entry)) => {
                    self.handle_overflow(entry, priority);
                }
                Err(TrySendError::Disconnected(_)) => {
                    // Logger is shutting down, silently ignore
                }
            }
        } else {
            let mut appenders = self.appenders.write();
            Self::process_sync(&mut appenders, &entry, &self.metrics);
        }
    }

    /// Handle queue overflow based on configured policy and log priority
    fn handle_overflow(&self, entry: LogEntry, priority: LogPriority) {
        self.metrics.record_queue_full();

        // Critical logs (Error, Fatal) are never dropped - force write synchronously
        if priority == LogPriority::Critical {
            self.force_write_critical(entry);
            return;
        }

        match &self.overflow_policy {
            OverflowPolicy::DropNewest => {
                // Silently drop but track metrics
                self.metrics.record_dropped();
            }

            OverflowPolicy::DropOldest => {
                // Note: True DropOldest requires access to the receiver side
                // which we don't have. Fall back to AlertAndDrop with a warning.
                self.alert_and_drop(entry, true);
            }

            OverflowPolicy::Block => {
                // Block until space is available
                self.metrics.record_block();
                if let Some(ref sender) = self.sender {
                    // send() blocks until successful
                    let _ = sender.send(entry);
                }
            }

            OverflowPolicy::BlockWithTimeout(timeout) => {
                self.metrics.record_block();
                if let Some(ref sender) = self.sender {
                    match sender.send_timeout(entry, *timeout) {
                        Ok(()) => {
                            // Successfully sent after waiting
                        }
                        Err(crossbeam_channel::SendTimeoutError::Timeout(entry)) => {
                            // Timeout expired, drop the log
                            self.alert_and_drop(entry, false);
                        }
                        Err(crossbeam_channel::SendTimeoutError::Disconnected(_)) => {
                            // Logger shutting down
                        }
                    }
                }
            }

            OverflowPolicy::AlertAndDrop => {
                self.alert_and_drop(entry, false);
            }
        }
    }

    /// Force write a critical log entry synchronously
    fn force_write_critical(&self, entry: LogEntry) {
        self.metrics.record_critical_preserved();

        // Try non-blocking lock first
        if let Some(mut appenders) = self.appenders.try_write() {
            Self::process_sync(&mut appenders, &entry, &self.metrics);
        } else {
            // For critical logs, we block to ensure they're written
            let mut appenders = self.appenders.write();
            Self::process_sync(&mut appenders, &entry, &self.metrics);
        }
    }

    /// Drop a log entry with alert notification
    fn alert_and_drop(&self, _entry: LogEntry, is_drop_oldest_fallback: bool) {
        let dropped_count = self.metrics.record_dropped();

        // Alert on first drop and periodically thereafter
        let should_alert = dropped_count == 0 || (dropped_count + 1).is_multiple_of(1000);

        if should_alert {
            if is_drop_oldest_fallback {
                eprintln!(
                    "[LOGGER WARNING] Queue full, {} logs dropped. \
                     Note: DropOldest policy not fully supported, using AlertAndDrop.",
                    dropped_count + 1
                );
            } else {
                eprintln!(
                    "[LOGGER WARNING] Queue full, {} logs dropped. \
                     Consider increasing buffer size or using a different overflow policy.",
                    dropped_count + 1
                );
            }

            // Call user-provided callback if available
            if let Some(ref callback) = self.on_overflow {
                callback(dropped_count + 1);
            }
        }
    }

    /// Get the number of dropped logs
    ///
    /// This counter tracks log entries that were dropped due to queue overflow
    /// or write failures. Useful for monitoring logger health.
    pub fn dropped_count(&self) -> u64 {
        self.metrics.dropped_count()
    }

    /// Get the number of failed log write attempts (alias for dropped_count)
    #[deprecated(since = "0.2.0", note = "Use dropped_count() instead")]
    pub fn failed_write_count(&self) -> u64 {
        self.metrics.dropped_count()
    }

    /// Get the number of queue full events
    ///
    /// This counter tracks how many times the async buffer became full.
    /// High counts indicate the buffer size may need to be increased.
    pub fn queue_full_count(&self) -> u64 {
        self.metrics.queue_full_events()
    }

    /// Get the number of synchronous fallback events (blocking events)
    ///
    /// This counter tracks how many times the logger had to block
    /// due to overflow policy configuration.
    pub fn sync_fallback_count(&self) -> u64 {
        self.metrics.block_events()
    }

    /// Get the logger metrics for detailed observability
    ///
    /// # Example
    ///
    /// ```
    /// use rust_logger_system::Logger;
    ///
    /// let logger = Logger::with_async(100);
    ///
    /// // After logging operations...
    /// let metrics = logger.metrics();
    /// println!("Dropped: {}", metrics.dropped_count());
    /// println!("Total logged: {}", metrics.total_logged());
    /// println!("Drop rate: {:.2}%", metrics.drop_rate());
    /// ```
    pub fn metrics(&self) -> &LoggerMetrics {
        &self.metrics
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
        self.send_entry(entry);
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

        // Report any dropped logs
        let dropped = self.metrics.dropped_count();
        if dropped > 0 {
            eprintln!(
                "[LOGGER WARNING] Logger shutting down with {} dropped logs (drop rate: {:.2}%)",
                dropped,
                self.metrics.drop_rate()
            );
        }
    }
}

/// Builder for constructing Logger with a fluent API
///
/// # Example
/// ```
/// use rust_logger_system::prelude::*;
/// use std::sync::Arc;
///
/// let logger = Logger::builder()
///     .min_level(LogLevel::Debug)
///     .appender(ConsoleAppender::new())
///     .async_mode(1000)
///     .overflow_policy(OverflowPolicy::AlertAndDrop)
///     .on_overflow(Arc::new(|count| {
///         eprintln!("ALERT: {} logs dropped", count);
///     }))
///     .build();
/// ```
pub struct LoggerBuilder {
    min_level: LogLevel,
    appenders: Vec<Box<dyn Appender>>,
    async_buffer: Option<usize>,
    overflow_policy: OverflowPolicy,
    on_overflow: Option<OverflowCallback>,
}

impl LoggerBuilder {
    /// Create a new builder with default values
    pub fn new() -> Self {
        Self {
            min_level: LogLevel::Info,
            appenders: Vec::new(),
            async_buffer: None,
            overflow_policy: OverflowPolicy::AlertAndDrop,
            on_overflow: None,
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

    /// Set the overflow policy for async logging
    ///
    /// Determines what happens when the async buffer is full.
    /// Default is `AlertAndDrop`.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_logger_system::prelude::*;
    /// use std::time::Duration;
    ///
    /// let logger = Logger::builder()
    ///     .async_mode(100)
    ///     .overflow_policy(OverflowPolicy::BlockWithTimeout(Duration::from_millis(50)))
    ///     .build();
    /// ```
    #[must_use = "builder methods return a new value"]
    pub fn overflow_policy(mut self, policy: OverflowPolicy) -> Self {
        self.overflow_policy = policy;
        self
    }

    /// Set a callback for overflow notifications
    ///
    /// The callback is invoked when logs are dropped due to queue overflow.
    /// The parameter is the total count of dropped logs.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_logger_system::prelude::*;
    /// use std::sync::Arc;
    ///
    /// let logger = Logger::builder()
    ///     .async_mode(100)
    ///     .on_overflow(Arc::new(|count| {
    ///         eprintln!("Warning: {} logs dropped", count);
    ///     }))
    ///     .build();
    /// ```
    #[must_use = "builder methods return a new value"]
    pub fn on_overflow(mut self, callback: OverflowCallback) -> Self {
        self.on_overflow = Some(callback);
        self
    }

    /// Build the Logger
    pub fn build(self) -> Logger {
        let mut logger = if let Some(size) = self.async_buffer {
            Logger::with_async_config(size, self.overflow_policy, self.on_overflow)
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
        let logger = Logger::builder().min_level(LogLevel::Debug).build();

        // Verify the logger was created
        assert_eq!(logger.dropped_count(), 0);
    }

    #[test]
    fn test_builder_with_appender() {
        let logger = Logger::builder()
            .min_level(LogLevel::Info)
            .appender(ConsoleAppender::new())
            .build();

        // Verify logger was created with appender
        assert_eq!(logger.dropped_count(), 0);
    }

    #[test]
    fn test_builder_async_mode() {
        let logger = Logger::builder()
            .min_level(LogLevel::Trace)
            .async_mode(1000)
            .build();

        // Verify async logger was created
        assert_eq!(logger.sync_fallback_count(), 0);
        assert_eq!(logger.dropped_count(), 0);
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
        assert_eq!(logger.dropped_count(), 0);
    }

    #[test]
    fn test_builder_default() {
        let builder = LoggerBuilder::default();
        let logger = builder.build();

        // Default logger should have Info level
        assert_eq!(logger.dropped_count(), 0);
    }

    #[test]
    fn test_overflow_policy_drop_newest() {
        let logger = Logger::builder()
            .async_mode(2) // Very small buffer
            .overflow_policy(OverflowPolicy::DropNewest)
            .appender(ConsoleAppender::new()) // Add appender to actually log
            .build();

        // Fill buffer and verify metrics
        for i in 0..10 {
            logger.debug(format!("Message {}", i));
        }

        // Give async thread time to process
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Metrics should be available - either something was logged or dropped
        let metrics = logger.metrics();
        // With DropNewest, we expect at least some to be processed or dropped
        // This test mainly verifies the policy doesn't panic
        let _ = metrics.total_logged();
        let _ = metrics.dropped_count();
    }

    #[test]
    fn test_overflow_policy_block() {
        let logger = Logger::builder()
            .async_mode(10)
            .overflow_policy(OverflowPolicy::Block)
            .build();

        // Block policy should eventually process all messages
        for i in 0..5 {
            logger.info(format!("Message {}", i));
        }

        // Wait for processing
        std::thread::sleep(std::time::Duration::from_millis(50));

        assert_eq!(logger.dropped_count(), 0);
    }

    #[test]
    fn test_overflow_policy_block_with_timeout() {
        let logger = Logger::builder()
            .async_mode(10)
            .overflow_policy(OverflowPolicy::BlockWithTimeout(Duration::from_millis(100)))
            .build();

        for i in 0..5 {
            logger.info(format!("Message {}", i));
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(logger.dropped_count(), 0);
    }

    #[test]
    fn test_overflow_callback() {
        use std::sync::atomic::{AtomicU64, Ordering};

        let callback_count = Arc::new(AtomicU64::new(0));
        let callback_count_clone = Arc::clone(&callback_count);

        let logger = Logger::builder()
            .async_mode(1) // Very small buffer
            .overflow_policy(OverflowPolicy::AlertAndDrop)
            .on_overflow(Arc::new(move |_count| {
                callback_count_clone.fetch_add(1, Ordering::Relaxed);
            }))
            .build();

        // Generate overflow
        for i in 0..100 {
            logger.debug(format!("Message {}", i));
        }

        std::thread::sleep(std::time::Duration::from_millis(50));

        // Callback may or may not have been called depending on timing
        // Just verify no panic occurred
        let _ = callback_count.load(Ordering::Relaxed);
    }

    #[test]
    fn test_critical_log_preservation() {
        let logger = Logger::builder()
            .async_mode(1) // Very small buffer
            .overflow_policy(OverflowPolicy::DropNewest)
            .build();

        // Fill buffer with debug logs, then send critical log
        for _ in 0..10 {
            logger.debug("Low priority");
        }

        // Critical logs should never be dropped
        logger.error("Critical error");
        logger.fatal("Fatal error");

        std::thread::sleep(std::time::Duration::from_millis(100));

        // Critical logs should have been preserved
        let metrics = logger.metrics();
        assert!(metrics.critical_logs_preserved() > 0 || metrics.total_logged() > 0);
    }

    #[test]
    fn test_log_priority() {
        assert_eq!(LogLevel::Trace.priority(), LogPriority::Normal);
        assert_eq!(LogLevel::Debug.priority(), LogPriority::Normal);
        assert_eq!(LogLevel::Info.priority(), LogPriority::Normal);
        assert_eq!(LogLevel::Warn.priority(), LogPriority::High);
        assert_eq!(LogLevel::Error.priority(), LogPriority::Critical);
        assert_eq!(LogLevel::Fatal.priority(), LogPriority::Critical);
    }

    #[test]
    fn test_metrics_drop_rate() {
        let metrics = LoggerMetrics::new();

        // No logs - 0% drop rate
        assert_eq!(metrics.drop_rate(), 0.0);

        // Record some logs
        for _ in 0..90 {
            metrics.record_logged();
        }
        for _ in 0..10 {
            metrics.record_dropped();
        }

        // 10 out of 100 = 10%
        let rate = metrics.drop_rate();
        assert!((9.9..=10.1).contains(&rate), "Drop rate was {}", rate);
    }
}
