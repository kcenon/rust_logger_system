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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    AlertAndDrop,
}

impl Default for OverflowPolicy {
    fn default() -> Self {
        OverflowPolicy::AlertAndDrop
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogPriority {
    /// Normal priority (Trace, Debug, Info)
    Normal = 0,
    /// High priority (Warn)
    High = 1,
    /// Critical priority (Error, Fatal) - never dropped
    Critical = 2,
}

impl Default for LogPriority {
    fn default() -> Self {
        LogPriority::Normal
    }
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
}
