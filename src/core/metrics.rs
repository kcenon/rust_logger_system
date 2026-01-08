//! Logger metrics for observability
//!
//! Provides counters and statistics for monitoring logger health,
//! including dropped log counts, queue overflow events, and throughput.

use std::sync::atomic::{AtomicU64, Ordering};

/// Metrics for logger observability
///
/// Tracks various statistics about logger operation, particularly
/// useful for detecting queue overflow and performance issues.
///
/// # Example
///
/// ```
/// use rust_logger_system::LoggerMetrics;
///
/// let metrics = LoggerMetrics::new();
///
/// // Record events
/// metrics.record_dropped();
/// metrics.record_logged();
///
/// // Check counts
/// assert_eq!(metrics.dropped_count(), 1);
/// assert_eq!(metrics.total_logged(), 1);
/// ```
#[derive(Debug)]
pub struct LoggerMetrics {
    /// Number of logs dropped due to queue overflow
    dropped_count: AtomicU64,

    /// Total number of logs successfully sent to queue or written
    total_logged: AtomicU64,

    /// Number of times the queue became full
    queue_full_events: AtomicU64,

    /// Number of times blocking occurred while waiting for queue space
    block_events: AtomicU64,

    /// Number of critical logs that were force-written
    critical_logs_preserved: AtomicU64,
}

impl LoggerMetrics {
    /// Create a new metrics instance with all counters at zero
    pub const fn new() -> Self {
        Self {
            dropped_count: AtomicU64::new(0),
            total_logged: AtomicU64::new(0),
            queue_full_events: AtomicU64::new(0),
            block_events: AtomicU64::new(0),
            critical_logs_preserved: AtomicU64::new(0),
        }
    }

    /// Get the number of dropped logs
    #[inline]
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count.load(Ordering::Relaxed)
    }

    /// Get the total number of logs processed
    #[inline]
    pub fn total_logged(&self) -> u64 {
        self.total_logged.load(Ordering::Relaxed)
    }

    /// Get the number of queue full events
    #[inline]
    pub fn queue_full_events(&self) -> u64 {
        self.queue_full_events.load(Ordering::Relaxed)
    }

    /// Get the number of blocking events
    #[inline]
    pub fn block_events(&self) -> u64 {
        self.block_events.load(Ordering::Relaxed)
    }

    /// Get the number of critical logs that were preserved
    #[inline]
    pub fn critical_logs_preserved(&self) -> u64 {
        self.critical_logs_preserved.load(Ordering::Relaxed)
    }

    /// Record a dropped log
    #[inline]
    pub fn record_dropped(&self) -> u64 {
        self.dropped_count.fetch_add(1, Ordering::Relaxed)
    }

    /// Record a successfully logged entry
    #[inline]
    pub fn record_logged(&self) -> u64 {
        self.total_logged.fetch_add(1, Ordering::Relaxed)
    }

    /// Record a queue full event
    #[inline]
    pub fn record_queue_full(&self) -> u64 {
        self.queue_full_events.fetch_add(1, Ordering::Relaxed)
    }

    /// Record a blocking event
    #[inline]
    pub fn record_block(&self) -> u64 {
        self.block_events.fetch_add(1, Ordering::Relaxed)
    }

    /// Record a preserved critical log
    #[inline]
    pub fn record_critical_preserved(&self) -> u64 {
        self.critical_logs_preserved.fetch_add(1, Ordering::Relaxed)
    }

    /// Get drop rate as a percentage (0.0 - 100.0)
    ///
    /// Returns 0.0 if no logs have been processed.
    pub fn drop_rate(&self) -> f64 {
        let dropped = self.dropped_count() as f64;
        let total = self.total_logged() as f64 + dropped;
        if total == 0.0 {
            0.0
        } else {
            (dropped / total) * 100.0
        }
    }

    /// Reset all metrics to zero
    ///
    /// Useful for testing or periodic reset of metrics.
    pub fn reset(&self) {
        self.dropped_count.store(0, Ordering::Relaxed);
        self.total_logged.store(0, Ordering::Relaxed);
        self.queue_full_events.store(0, Ordering::Relaxed);
        self.block_events.store(0, Ordering::Relaxed);
        self.critical_logs_preserved.store(0, Ordering::Relaxed);
    }
}

impl Default for LoggerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for LoggerMetrics {
    /// Create a snapshot of the current metrics values
    fn clone(&self) -> Self {
        Self {
            dropped_count: AtomicU64::new(self.dropped_count()),
            total_logged: AtomicU64::new(self.total_logged()),
            queue_full_events: AtomicU64::new(self.queue_full_events()),
            block_events: AtomicU64::new(self.block_events()),
            critical_logs_preserved: AtomicU64::new(self.critical_logs_preserved()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_new() {
        let metrics = LoggerMetrics::new();
        assert_eq!(metrics.dropped_count(), 0);
        assert_eq!(metrics.total_logged(), 0);
        assert_eq!(metrics.queue_full_events(), 0);
        assert_eq!(metrics.block_events(), 0);
        assert_eq!(metrics.critical_logs_preserved(), 0);
    }

    #[test]
    fn test_metrics_record_dropped() {
        let metrics = LoggerMetrics::new();
        assert_eq!(metrics.record_dropped(), 0); // Returns previous value
        assert_eq!(metrics.dropped_count(), 1);
        metrics.record_dropped();
        assert_eq!(metrics.dropped_count(), 2);
    }

    #[test]
    fn test_metrics_record_logged() {
        let metrics = LoggerMetrics::new();
        metrics.record_logged();
        metrics.record_logged();
        assert_eq!(metrics.total_logged(), 2);
    }

    #[test]
    fn test_metrics_drop_rate() {
        let metrics = LoggerMetrics::new();

        // No logs - 0% drop rate
        assert_eq!(metrics.drop_rate(), 0.0);

        // 100 logged, 0 dropped - 0% drop rate
        for _ in 0..100 {
            metrics.record_logged();
        }
        assert_eq!(metrics.drop_rate(), 0.0);

        // 100 logged, 10 dropped - ~9.09% drop rate
        for _ in 0..10 {
            metrics.record_dropped();
        }
        let rate = metrics.drop_rate();
        assert!(rate > 9.0 && rate < 10.0, "Drop rate was {}", rate);
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = LoggerMetrics::new();
        metrics.record_dropped();
        metrics.record_logged();
        metrics.record_queue_full();

        metrics.reset();

        assert_eq!(metrics.dropped_count(), 0);
        assert_eq!(metrics.total_logged(), 0);
        assert_eq!(metrics.queue_full_events(), 0);
    }

    #[test]
    fn test_metrics_clone() {
        let metrics = LoggerMetrics::new();
        metrics.record_dropped();
        metrics.record_logged();
        metrics.record_logged();

        let snapshot = metrics.clone();
        assert_eq!(snapshot.dropped_count(), 1);
        assert_eq!(snapshot.total_logged(), 2);

        // Original and clone are independent
        metrics.record_dropped();
        assert_eq!(metrics.dropped_count(), 2);
        assert_eq!(snapshot.dropped_count(), 1);
    }
}
