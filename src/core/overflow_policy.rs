//! Overflow policies for async logging queue management
//!
//! When the async logging queue is full, these policies determine how
//! to handle new log entries to prevent silent log loss.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// Policy for handling queue overflow in async logging
///
/// When the async logging buffer is full, this policy determines
/// what action to take with new log entries.
///
/// # Example
///
/// ```
/// use rust_logger_system::OverflowPolicy;
/// use std::time::Duration;
///
/// // Default behavior: alert and drop
/// let policy = OverflowPolicy::default();
///
/// // Block with timeout
/// let policy = OverflowPolicy::BlockWithTimeout(Duration::from_millis(100));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum OverflowPolicy {
    /// Drop new logs when queue is full (current behavior)
    ///
    /// Logs are silently dropped but metrics are tracked.
    /// Use this for high-throughput scenarios where some log loss is acceptable.
    DropNewest,

    /// Drop oldest logs to make room for new ones
    ///
    /// Note: Due to channel implementation limitations, this policy
    /// falls back to `AlertAndDrop` behavior with an additional warning.
    /// True FIFO eviction would require a different queue implementation.
    DropOldest,

    /// Block until space is available
    ///
    /// Warning: This can cause backpressure in the application.
    /// Only use when log preservation is critical and you can tolerate latency.
    Block,

    /// Block with timeout, then drop
    ///
    /// Attempts to wait for space, but drops if timeout expires.
    /// Good balance between preservation and responsiveness.
    BlockWithTimeout(Duration),

    /// Drop but alert via callback and stderr
    ///
    /// This is the recommended default. Logs are dropped when queue is full,
    /// but operators are alerted so they can take action.
    #[default]
    AlertAndDrop,
}

impl fmt::Display for OverflowPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OverflowPolicy::DropNewest => write!(f, "DropNewest"),
            OverflowPolicy::DropOldest => write!(f, "DropOldest"),
            OverflowPolicy::Block => write!(f, "Block"),
            OverflowPolicy::BlockWithTimeout(d) => write!(f, "BlockWithTimeout({:?})", d),
            OverflowPolicy::AlertAndDrop => write!(f, "AlertAndDrop"),
        }
    }
}

/// Priority level for log preservation during overflow
///
/// Higher priority logs are preserved over lower priority ones
/// when the queue is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum LogPriority {
    /// Normal priority (Trace, Debug, Info)
    #[default]
    Normal = 0,
    /// High priority (Warn)
    High = 1,
    /// Critical priority (Error, Fatal) - never dropped
    Critical = 2,
}

impl fmt::Display for LogPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogPriority::Normal => write!(f, "Normal"),
            LogPriority::High => write!(f, "High"),
            LogPriority::Critical => write!(f, "Critical"),
        }
    }
}

/// Callback type for overflow notifications
///
/// Called when logs are dropped due to queue overflow.
/// The parameter is the total count of dropped logs so far.
pub type OverflowCallback = Arc<dyn Fn(u64) + Send + Sync>;

/// Configuration for priority-based log preservation
///
/// This configuration allows customization of how different log priorities
/// are handled when the async queue is full.
///
/// # Example
///
/// ```
/// use rust_logger_system::PriorityConfig;
///
/// let config = PriorityConfig::default();
/// assert!(config.preserve_critical);
/// assert!(config.preserve_high);
///
/// // Custom configuration
/// let config = PriorityConfig {
///     preserve_critical: true,
///     preserve_high: true,
///     block_on_critical: true,
///     high_priority_retry_count: 3,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorityConfig {
    /// Whether to preserve critical logs (Error, Fatal) - never drop them
    ///
    /// When true, critical logs are written synchronously if the queue is full.
    /// Default: true
    pub preserve_critical: bool,

    /// Whether to preserve high priority logs (Warn) when possible
    ///
    /// When true, high priority logs get additional retry attempts before dropping.
    /// Default: true
    pub preserve_high: bool,

    /// Whether to block the calling thread for critical logs when queue is full
    ///
    /// When true (default), critical logs will block until they can be written.
    /// When false, critical logs are written synchronously but don't block the sender.
    /// Default: true
    pub block_on_critical: bool,

    /// Number of retry attempts for high priority logs before applying overflow policy
    ///
    /// Only used when `preserve_high` is true.
    /// Default: 3
    pub high_priority_retry_count: u32,
}

impl Default for PriorityConfig {
    fn default() -> Self {
        Self {
            preserve_critical: true,
            preserve_high: true,
            block_on_critical: true,
            high_priority_retry_count: 3,
        }
    }
}

impl fmt::Display for PriorityConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PriorityConfig {{ preserve_critical: {}, preserve_high: {}, block_on_critical: {}, high_priority_retry_count: {} }}",
            self.preserve_critical,
            self.preserve_high,
            self.block_on_critical,
            self.high_priority_retry_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overflow_policy_default() {
        let policy = OverflowPolicy::default();
        assert_eq!(policy, OverflowPolicy::AlertAndDrop);
    }

    #[test]
    fn test_overflow_policy_display() {
        assert_eq!(OverflowPolicy::DropNewest.to_string(), "DropNewest");
        assert_eq!(OverflowPolicy::DropOldest.to_string(), "DropOldest");
        assert_eq!(OverflowPolicy::Block.to_string(), "Block");
        assert_eq!(
            OverflowPolicy::BlockWithTimeout(Duration::from_millis(100)).to_string(),
            "BlockWithTimeout(100ms)"
        );
        assert_eq!(OverflowPolicy::AlertAndDrop.to_string(), "AlertAndDrop");
    }

    #[test]
    fn test_log_priority_ordering() {
        assert!(LogPriority::Normal < LogPriority::High);
        assert!(LogPriority::High < LogPriority::Critical);
        assert!(LogPriority::Normal < LogPriority::Critical);
    }

    #[test]
    fn test_log_priority_default() {
        assert_eq!(LogPriority::default(), LogPriority::Normal);
    }

    #[test]
    fn test_priority_config_default() {
        let config = PriorityConfig::default();
        assert!(config.preserve_critical);
        assert!(config.preserve_high);
        assert!(config.block_on_critical);
        assert_eq!(config.high_priority_retry_count, 3);
    }

    #[test]
    fn test_priority_config_display() {
        let config = PriorityConfig::default();
        let display = config.to_string();
        assert!(display.contains("preserve_critical: true"));
        assert!(display.contains("preserve_high: true"));
        assert!(display.contains("block_on_critical: true"));
        assert!(display.contains("high_priority_retry_count: 3"));
    }

    #[test]
    fn test_priority_config_custom() {
        let config = PriorityConfig {
            preserve_critical: true,
            preserve_high: false,
            block_on_critical: false,
            high_priority_retry_count: 5,
        };
        assert!(config.preserve_critical);
        assert!(!config.preserve_high);
        assert!(!config.block_on_critical);
        assert_eq!(config.high_priority_retry_count, 5);
    }
}
