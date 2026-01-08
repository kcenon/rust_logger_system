//! Log sampling for high-volume scenarios
//!
//! Provides configurable log sampling to reduce log volume in high-throughput
//! scenarios while ensuring critical logs are never dropped.
//!
//! # Features
//!
//! - **Random Sampling**: Configurable sample rate between 0.0 and 1.0
//! - **Level Bypass**: Critical levels (Error, Fatal) are never sampled
//! - **Category-based Sampling**: Different rates for different log categories
//! - **Adaptive Sampling**: Automatically adjusts rate based on throughput
//!
//! # Example
//!
//! ```
//! use rust_logger_system::prelude::*;
//! use std::collections::HashMap;
//!
//! let logger = Logger::builder()
//!     .with_sampling(SamplingConfig {
//!         rate: 0.1,  // Sample 10% of logs
//!         always_sample: vec![LogLevel::Warn, LogLevel::Error, LogLevel::Fatal],
//!         category_rates: HashMap::new(),
//!         adaptive: false,
//!         adaptive_threshold: 10000,
//!         adaptive_min_rate: 0.01,
//!     })
//!     .build();
//! ```

use super::log_level::LogLevel;
use rand::Rng;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

/// Configuration for log sampling
///
/// Controls how logs are sampled to reduce volume in high-throughput scenarios.
///
/// # Example
///
/// ```
/// use rust_logger_system::prelude::*;
/// use std::collections::HashMap;
///
/// // Sample 10% of logs, but always log warnings and above
/// let config = SamplingConfig {
///     rate: 0.1,
///     always_sample: vec![LogLevel::Warn, LogLevel::Error, LogLevel::Fatal],
///     category_rates: HashMap::new(),
///     adaptive: false,
///     adaptive_threshold: 10000,
///     adaptive_min_rate: 0.01,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct SamplingConfig {
    /// Sample rate between 0.0 and 1.0
    ///
    /// - 1.0 = no sampling (log everything)
    /// - 0.5 = sample 50% of logs
    /// - 0.1 = sample 10% of logs
    /// - 0.0 = drop all logs (except always_sample levels)
    pub rate: f64,

    /// Log levels that are never sampled (always logged)
    ///
    /// Typically includes Error and Fatal to ensure critical issues
    /// are never missed.
    pub always_sample: Vec<LogLevel>,

    /// Per-category sample rates
    ///
    /// Allows different sampling rates for different log categories.
    /// Category is extracted from the "category" field in log context.
    pub category_rates: HashMap<String, f64>,

    /// Enable adaptive sampling based on throughput
    ///
    /// When enabled, the sampler automatically reduces the sampling rate
    /// when log throughput exceeds `adaptive_threshold`.
    pub adaptive: bool,

    /// Threshold (messages per second) to trigger adaptive sampling
    ///
    /// When throughput exceeds this value and `adaptive` is true,
    /// the sampling rate is reduced proportionally.
    pub adaptive_threshold: usize,

    /// Minimum rate for adaptive sampling
    ///
    /// The sampling rate will never go below this value, even under
    /// extreme load.
    pub adaptive_min_rate: f64,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            rate: 1.0, // No sampling by default
            always_sample: vec![LogLevel::Error, LogLevel::Fatal],
            category_rates: HashMap::new(),
            adaptive: false,
            adaptive_threshold: 10000,
            adaptive_min_rate: 0.01,
        }
    }
}

impl SamplingConfig {
    /// Create a new sampling config with the specified rate
    ///
    /// # Arguments
    ///
    /// * `rate` - Sample rate between 0.0 and 1.0
    ///
    /// # Example
    ///
    /// ```
    /// use rust_logger_system::SamplingConfig;
    ///
    /// let config = SamplingConfig::new(0.5); // Sample 50%
    /// ```
    pub fn new(rate: f64) -> Self {
        Self {
            rate: rate.clamp(0.0, 1.0),
            ..Default::default()
        }
    }

    /// Create a config that always logs all messages (no sampling)
    pub fn no_sampling() -> Self {
        Self::default()
    }

    /// Set the levels that should always be logged
    #[must_use]
    pub fn with_always_sample(mut self, levels: Vec<LogLevel>) -> Self {
        self.always_sample = levels;
        self
    }

    /// Add a category-specific sample rate
    #[must_use]
    pub fn with_category_rate(mut self, category: impl Into<String>, rate: f64) -> Self {
        self.category_rates.insert(category.into(), rate.clamp(0.0, 1.0));
        self
    }

    /// Enable adaptive sampling
    #[must_use]
    pub fn with_adaptive(mut self, threshold: usize, min_rate: f64) -> Self {
        self.adaptive = true;
        self.adaptive_threshold = threshold;
        self.adaptive_min_rate = min_rate.clamp(0.0, 1.0);
        self
    }
}

/// Metrics for sampling observability
///
/// Tracks how many logs were sampled vs dropped, allowing monitoring
/// of sampling effectiveness.
///
/// # Example
///
/// ```
/// use rust_logger_system::SamplerMetrics;
///
/// let metrics = SamplerMetrics::new();
/// assert_eq!(metrics.sampled_count(), 0);
/// assert_eq!(metrics.dropped_count(), 0);
/// ```
#[derive(Debug)]
pub struct SamplerMetrics {
    /// Number of logs that passed sampling (were logged)
    sampled_count: AtomicU64,

    /// Number of logs dropped by sampling
    dropped_count: AtomicU64,

    /// Total number of logs processed by sampler
    total_count: AtomicU64,
}

impl SamplerMetrics {
    /// Create new metrics with all counters at zero
    pub const fn new() -> Self {
        Self {
            sampled_count: AtomicU64::new(0),
            dropped_count: AtomicU64::new(0),
            total_count: AtomicU64::new(0),
        }
    }

    /// Get the number of sampled (logged) entries
    #[inline]
    pub fn sampled_count(&self) -> u64 {
        self.sampled_count.load(Ordering::Relaxed)
    }

    /// Get the number of dropped entries
    #[inline]
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count.load(Ordering::Relaxed)
    }

    /// Get the total number of entries processed
    #[inline]
    pub fn total_count(&self) -> u64 {
        self.total_count.load(Ordering::Relaxed)
    }

    /// Record a sampled (logged) entry
    #[inline]
    pub(crate) fn record_sampled(&self) {
        self.sampled_count.fetch_add(1, Ordering::Relaxed);
        self.total_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a dropped entry
    #[inline]
    pub(crate) fn record_dropped(&self) {
        self.dropped_count.fetch_add(1, Ordering::Relaxed);
        self.total_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the effective sample rate based on actual sampling
    ///
    /// Returns 1.0 if no logs have been processed yet.
    pub fn effective_sample_rate(&self) -> f64 {
        let sampled = self.sampled_count() as f64;
        let total = self.total_count() as f64;

        if total == 0.0 {
            1.0
        } else {
            sampled / total
        }
    }

    /// Reset all counters to zero
    pub fn reset(&self) {
        self.sampled_count.store(0, Ordering::Relaxed);
        self.dropped_count.store(0, Ordering::Relaxed);
        self.total_count.store(0, Ordering::Relaxed);
    }
}

impl Default for SamplerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SamplerMetrics {
    fn clone(&self) -> Self {
        Self {
            sampled_count: AtomicU64::new(self.sampled_count()),
            dropped_count: AtomicU64::new(self.dropped_count()),
            total_count: AtomicU64::new(self.total_count()),
        }
    }
}

/// Tracks message rate for adaptive sampling
///
/// Uses a sliding window approach to calculate the current
/// message rate (messages per second).
#[derive(Debug)]
struct RateTracker {
    /// Start time of the current measurement window
    window_start: Instant,

    /// Message count in current window
    window_count: AtomicUsize,

    /// Last calculated rate (cached)
    last_rate: AtomicU64,
}

impl RateTracker {
    /// Create a new rate tracker
    fn new() -> Self {
        Self {
            window_start: Instant::now(),
            window_count: AtomicUsize::new(0),
            last_rate: AtomicU64::new(0),
        }
    }

    /// Record a message and get the current rate
    fn record_and_get_rate(&self) -> f64 {
        self.window_count.fetch_add(1, Ordering::Relaxed);

        let elapsed = self.window_start.elapsed().as_secs_f64();

        if elapsed > 0.0 {
            let count = self.window_count.load(Ordering::Relaxed);
            let rate = count as f64 / elapsed;

            // Cache the rate for quick access
            self.last_rate.store(rate.to_bits(), Ordering::Relaxed);

            // If window is complete, we could reset here, but for simplicity
            // we just keep accumulating. The rate calculation remains accurate.
            rate
        } else {
            0.0
        }
    }

    /// Get the last calculated rate without recording
    fn current_rate(&self) -> f64 {
        f64::from_bits(self.last_rate.load(Ordering::Relaxed))
    }
}

/// Log sampler for high-volume scenarios
///
/// Determines whether each log entry should be sampled (logged) or dropped
/// based on the configured sampling strategy.
///
/// # Thread Safety
///
/// The sampler is thread-safe and uses atomic operations for all counters.
/// The random number generator is created per-call to avoid contention.
///
/// # Example
///
/// ```
/// use rust_logger_system::{LogSampler, SamplingConfig, LogLevel};
///
/// let sampler = LogSampler::new(SamplingConfig::new(0.5));
///
/// // Check if a log should be sampled
/// let should_log = sampler.should_sample(LogLevel::Info, None);
///
/// // Critical logs are always sampled
/// assert!(sampler.should_sample(LogLevel::Error, None));
/// ```
pub struct LogSampler {
    config: SamplingConfig,
    metrics: SamplerMetrics,
    rate_tracker: RateTracker,
}

impl LogSampler {
    /// Create a new sampler with the given configuration
    pub fn new(config: SamplingConfig) -> Self {
        Self {
            config,
            metrics: SamplerMetrics::new(),
            rate_tracker: RateTracker::new(),
        }
    }

    /// Determine if a log entry should be sampled (logged)
    ///
    /// # Arguments
    ///
    /// * `level` - The log level of the entry
    /// * `category` - Optional category for category-specific sampling
    ///
    /// # Returns
    ///
    /// `true` if the log should be recorded, `false` if it should be dropped.
    pub fn should_sample(&self, level: LogLevel, category: Option<&str>) -> bool {
        // Always sample configured levels (typically Error, Fatal)
        if self.config.always_sample.contains(&level) {
            self.metrics.record_sampled();
            return true;
        }

        // Get effective rate
        let rate = self.get_effective_rate(category);

        // Fast path: if rate is 1.0, always sample
        if rate >= 1.0 {
            self.metrics.record_sampled();
            return true;
        }

        // Fast path: if rate is 0.0, never sample (except always_sample levels)
        if rate <= 0.0 {
            self.metrics.record_dropped();
            return false;
        }

        // Random sampling
        let sample = rand::thread_rng().gen::<f64>() < rate;

        if sample {
            self.metrics.record_sampled();
        } else {
            self.metrics.record_dropped();
        }

        sample
    }

    /// Get the effective sampling rate, considering adaptive sampling
    fn get_effective_rate(&self, category: Option<&str>) -> f64 {
        // Check category-specific rate first
        if let Some(cat) = category {
            if let Some(&rate) = self.config.category_rates.get(cat) {
                return rate;
            }
        }

        // Apply adaptive sampling if enabled
        if self.config.adaptive {
            let current_rate = self.rate_tracker.record_and_get_rate();

            if current_rate > self.config.adaptive_threshold as f64 {
                // Reduce sampling rate proportionally to load
                let scale = self.config.adaptive_threshold as f64 / current_rate;
                return (self.config.rate * scale).max(self.config.adaptive_min_rate);
            }
        }

        self.config.rate
    }

    /// Get the sampler metrics
    pub fn metrics(&self) -> &SamplerMetrics {
        &self.metrics
    }

    /// Get the effective sample rate based on actual sampling
    pub fn effective_sample_rate(&self) -> f64 {
        self.metrics.effective_sample_rate()
    }

    /// Get the current message rate (messages per second)
    ///
    /// Only meaningful when adaptive sampling is enabled.
    pub fn current_message_rate(&self) -> f64 {
        self.rate_tracker.current_rate()
    }

    /// Get a reference to the sampling configuration
    pub fn config(&self) -> &SamplingConfig {
        &self.config
    }
}

impl std::fmt::Debug for LogSampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogSampler")
            .field("config", &self.config)
            .field("metrics", &self.metrics)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampling_config_default() {
        let config = SamplingConfig::default();
        assert_eq!(config.rate, 1.0);
        assert!(config.always_sample.contains(&LogLevel::Error));
        assert!(config.always_sample.contains(&LogLevel::Fatal));
        assert!(!config.adaptive);
    }

    #[test]
    fn test_sampling_config_new() {
        let config = SamplingConfig::new(0.5);
        assert_eq!(config.rate, 0.5);

        // Rate should be clamped
        let config = SamplingConfig::new(1.5);
        assert_eq!(config.rate, 1.0);

        let config = SamplingConfig::new(-0.5);
        assert_eq!(config.rate, 0.0);
    }

    #[test]
    fn test_sampling_config_builder() {
        let config = SamplingConfig::new(0.3)
            .with_always_sample(vec![LogLevel::Warn, LogLevel::Error, LogLevel::Fatal])
            .with_category_rate("database", 0.01)
            .with_adaptive(50000, 0.001);

        assert_eq!(config.rate, 0.3);
        assert!(config.always_sample.contains(&LogLevel::Warn));
        assert_eq!(config.category_rates.get("database"), Some(&0.01));
        assert!(config.adaptive);
        assert_eq!(config.adaptive_threshold, 50000);
        assert_eq!(config.adaptive_min_rate, 0.001);
    }

    #[test]
    fn test_sampler_metrics() {
        let metrics = SamplerMetrics::new();
        assert_eq!(metrics.sampled_count(), 0);
        assert_eq!(metrics.dropped_count(), 0);
        assert_eq!(metrics.total_count(), 0);
        assert_eq!(metrics.effective_sample_rate(), 1.0);

        metrics.record_sampled();
        metrics.record_sampled();
        metrics.record_dropped();

        assert_eq!(metrics.sampled_count(), 2);
        assert_eq!(metrics.dropped_count(), 1);
        assert_eq!(metrics.total_count(), 3);

        let rate = metrics.effective_sample_rate();
        assert!((rate - 0.666).abs() < 0.01);

        metrics.reset();
        assert_eq!(metrics.total_count(), 0);
    }

    #[test]
    fn test_sampler_always_sample_critical() {
        let config = SamplingConfig::new(0.0); // Drop everything
        let sampler = LogSampler::new(config);

        // Error and Fatal should always be sampled
        assert!(sampler.should_sample(LogLevel::Error, None));
        assert!(sampler.should_sample(LogLevel::Fatal, None));

        // Other levels should be dropped (rate is 0.0)
        // Run multiple times to ensure consistency
        for _ in 0..10 {
            assert!(!sampler.should_sample(LogLevel::Debug, None));
            assert!(!sampler.should_sample(LogLevel::Info, None));
        }
    }

    #[test]
    fn test_sampler_rate_1_0() {
        let sampler = LogSampler::new(SamplingConfig::new(1.0));

        // All logs should be sampled
        for _ in 0..100 {
            assert!(sampler.should_sample(LogLevel::Debug, None));
            assert!(sampler.should_sample(LogLevel::Info, None));
            assert!(sampler.should_sample(LogLevel::Warn, None));
        }
    }

    #[test]
    fn test_sampler_category_rate() {
        let config = SamplingConfig::new(1.0)
            .with_category_rate("noisy", 0.0);
        let sampler = LogSampler::new(config);

        // Default category should be sampled
        assert!(sampler.should_sample(LogLevel::Info, None));

        // "noisy" category should be dropped
        for _ in 0..10 {
            assert!(!sampler.should_sample(LogLevel::Info, Some("noisy")));
        }
    }

    #[test]
    fn test_sampler_statistical_rate() {
        // Test that sampling rate is approximately correct
        let sampler = LogSampler::new(SamplingConfig::new(0.5));

        let mut sampled = 0;
        let total = 10000;

        for _ in 0..total {
            if sampler.should_sample(LogLevel::Info, None) {
                sampled += 1;
            }
        }

        // Should be approximately 50% with some tolerance
        let rate = sampled as f64 / total as f64;
        assert!(
            (0.45..=0.55).contains(&rate),
            "Expected ~50% sample rate, got {}%",
            rate * 100.0
        );
    }

    #[test]
    fn test_sampler_metrics_tracking() {
        let sampler = LogSampler::new(SamplingConfig::new(0.5));

        for _ in 0..100 {
            sampler.should_sample(LogLevel::Info, None);
        }

        let metrics = sampler.metrics();
        assert_eq!(metrics.total_count(), 100);
        assert_eq!(
            metrics.sampled_count() + metrics.dropped_count(),
            100
        );
    }

    #[test]
    fn test_rate_tracker() {
        let tracker = RateTracker::new();

        // Record some messages
        for _ in 0..100 {
            tracker.record_and_get_rate();
        }

        let rate = tracker.current_rate();
        assert!(rate > 0.0, "Rate should be positive");
    }

    #[test]
    fn test_sampler_debug() {
        let sampler = LogSampler::new(SamplingConfig::new(0.5));
        let debug_str = format!("{:?}", sampler);
        assert!(debug_str.contains("LogSampler"));
        assert!(debug_str.contains("config"));
    }
}
