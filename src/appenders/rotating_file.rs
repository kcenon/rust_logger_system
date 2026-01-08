//! Rotating file appender with automatic log rotation
//!
//! This module provides a file appender that automatically rotates log files
//! based on various strategies including size, time, daily, hourly, or hybrid.

use crate::core::appender::Appender;
use crate::core::error::{LoggerError, Result};
use crate::core::log_entry::LogEntry;
use crate::core::timestamp::TimestampFormat;
use chrono::{DateTime, Local, Timelike};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Rotation strategy defining when to rotate log files
///
/// # Examples
///
/// ```
/// use rust_logger_system::appenders::RotationStrategy;
/// use std::time::Duration;
///
/// // Rotate when file exceeds 100 MB
/// let size_strategy = RotationStrategy::Size { max_bytes: 100 * 1024 * 1024 };
///
/// // Rotate every hour
/// let time_strategy = RotationStrategy::Time { interval: Duration::from_secs(3600) };
///
/// // Rotate daily at midnight
/// let daily_strategy = RotationStrategy::Daily { hour: 0 };
///
/// // Rotate on size OR time, whichever comes first
/// let hybrid_strategy = RotationStrategy::Hybrid {
///     max_bytes: 50 * 1024 * 1024,
///     interval: Duration::from_secs(24 * 3600),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum RotationStrategy {
    /// Rotate when file exceeds size in bytes
    Size { max_bytes: u64 },

    /// Rotate at time interval
    Time { interval: Duration },

    /// Rotate daily at specified hour (0-23)
    Daily { hour: u8 },

    /// Rotate hourly
    Hourly,

    /// Rotate on size OR time, whichever comes first
    Hybrid { max_bytes: u64, interval: Duration },

    /// No rotation (useful for testing or when external rotation is used)
    Never,
}

impl Default for RotationStrategy {
    fn default() -> Self {
        RotationStrategy::Size {
            max_bytes: 10 * 1024 * 1024, // 10 MB
        }
    }
}

impl RotationStrategy {
    /// Create a size-based rotation strategy
    #[must_use]
    pub fn size(max_bytes: u64) -> Self {
        RotationStrategy::Size { max_bytes }
    }

    /// Create a time-based rotation strategy
    #[must_use]
    pub fn time(interval: Duration) -> Self {
        RotationStrategy::Time { interval }
    }

    /// Create a daily rotation strategy
    ///
    /// # Panics
    ///
    /// Panics if hour is greater than 23
    #[must_use]
    pub fn daily(hour: u8) -> Self {
        assert!(hour <= 23, "Hour must be between 0 and 23");
        RotationStrategy::Daily { hour }
    }

    /// Create an hourly rotation strategy
    #[must_use]
    pub fn hourly() -> Self {
        RotationStrategy::Hourly
    }

    /// Create a hybrid rotation strategy (size OR time)
    #[must_use]
    pub fn hybrid(max_bytes: u64, interval: Duration) -> Self {
        RotationStrategy::Hybrid { max_bytes, interval }
    }

    /// Create a never-rotate strategy
    #[must_use]
    pub fn never() -> Self {
        RotationStrategy::Never
    }
}

/// Configuration for rotating file appender
///
/// # Examples
///
/// ```
/// use rust_logger_system::appenders::{RotationPolicy, RotationStrategy};
/// use std::time::Duration;
///
/// // Size-based rotation with compression
/// let policy = RotationPolicy::new()
///     .with_strategy(RotationStrategy::Size { max_bytes: 50 * 1024 * 1024 })
///     .with_max_backups(7)
///     .with_compression(true);
///
/// // Daily rotation at 2 AM
/// let policy = RotationPolicy::new()
///     .with_strategy(RotationStrategy::Daily { hour: 2 })
///     .with_max_backups(30)
///     .with_compression(true);
/// ```
#[derive(Debug, Clone)]
pub struct RotationPolicy {
    /// Rotation strategy defining when to rotate
    pub strategy: RotationStrategy,
    /// Maximum number of rotated files to keep
    pub max_backup_files: usize,
    /// Whether to compress rotated files
    pub compress: bool,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            strategy: RotationStrategy::default(),
            max_backup_files: 5,
            compress: false,
        }
    }
}

impl RotationPolicy {
    /// Create a new rotation policy with default settings
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the rotation strategy
    #[must_use = "builder methods return a new value and do not modify the original"]
    pub fn with_strategy(mut self, strategy: RotationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set maximum file size (convenience method for size-based rotation)
    ///
    /// This is equivalent to `with_strategy(RotationStrategy::Size { max_bytes: size })`
    #[must_use = "builder methods return a new value and do not modify the original"]
    pub fn with_max_size(mut self, size: u64) -> Self {
        self.strategy = RotationStrategy::Size { max_bytes: size };
        self
    }

    /// Set maximum backup files
    #[must_use = "builder methods return a new value and do not modify the original"]
    pub fn with_max_backups(mut self, count: usize) -> Self {
        self.max_backup_files = count;
        self
    }

    /// Enable compression
    #[must_use = "builder methods return a new value and do not modify the original"]
    pub fn with_compression(mut self, enabled: bool) -> Self {
        self.compress = enabled;
        self
    }

    /// Get the maximum file size if using size-based rotation
    ///
    /// Returns `None` if the strategy doesn't include size-based rotation.
    #[must_use]
    pub fn max_file_size(&self) -> Option<u64> {
        match &self.strategy {
            RotationStrategy::Size { max_bytes } => Some(*max_bytes),
            RotationStrategy::Hybrid { max_bytes, .. } => Some(*max_bytes),
            _ => None,
        }
    }
}

/// Rotating file appender with support for multiple rotation strategies
///
/// # Examples
///
/// ```no_run
/// use rust_logger_system::appenders::{RotatingFileAppender, RotationPolicy, RotationStrategy};
/// use std::time::Duration;
///
/// // Create appender with size-based rotation (default)
/// let appender = RotatingFileAppender::new("/var/log/app.log").unwrap();
///
/// // Create appender with time-based rotation
/// let policy = RotationPolicy::new()
///     .with_strategy(RotationStrategy::Time { interval: Duration::from_secs(3600) })
///     .with_max_backups(24);
/// let appender = RotatingFileAppender::with_policy("/var/log/app.log", policy).unwrap();
///
/// // Create appender with daily rotation at midnight
/// let policy = RotationPolicy::new()
///     .with_strategy(RotationStrategy::Daily { hour: 0 })
///     .with_max_backups(7)
///     .with_compression(true);
/// let appender = RotatingFileAppender::with_policy("/var/log/app.log", policy).unwrap();
/// ```
pub struct RotatingFileAppender {
    base_path: PathBuf,
    policy: RotationPolicy,
    writer: Option<BufWriter<File>>,
    current_size: u64,
    /// Timestamp of the last rotation (used for time-based strategies)
    last_rotation: SystemTime,
    /// Counter for consecutive deletion failures (reset on successful deletion)
    deletion_failure_count: usize,
    /// Timestamp format for log entries
    timestamp_format: TimestampFormat,
}

impl RotatingFileAppender {
    /// Create a new rotating file appender
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be created or opened
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::with_policy(path, RotationPolicy::default())
    }

    /// Create a new rotating file appender with custom policy
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be created or opened
    pub fn with_policy<P: AsRef<Path>>(path: P, policy: RotationPolicy) -> Result<Self> {
        let base_path = path.as_ref().to_path_buf();

        // Create parent directory if it doesn't exist
        if let Some(parent) = base_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                LoggerError::io_operation(
                    "create log directory",
                    format!("Failed to create directory '{}'", parent.display()),
                    e,
                )
            })?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&base_path)
            .map_err(|e| {
                LoggerError::file_appender(
                    base_path.display().to_string(),
                    format!("Failed to open: {}", e),
                )
            })?;

        let metadata = file.metadata()
            .map_err(|e| LoggerError::file_appender(
                base_path.display().to_string(),
                format!("Cannot access file metadata: {}", e)
            ))?;
        let current_size = metadata.len();

        // Use file modification time as last rotation time, or current time if unavailable
        let last_rotation = metadata.modified().unwrap_or_else(|_| SystemTime::now());
        let writer = Some(BufWriter::new(file));

        Ok(Self {
            base_path,
            policy,
            writer,
            current_size,
            last_rotation,
            deletion_failure_count: 0,
            timestamp_format: TimestampFormat::default(),
        })
    }

    /// Set the timestamp format for this appender
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use rust_logger_system::appenders::RotatingFileAppender;
    /// use rust_logger_system::TimestampFormat;
    ///
    /// let appender = RotatingFileAppender::new("/var/log/app.log")
    ///     .unwrap()
    ///     .with_timestamp_format(TimestampFormat::UnixMillis);
    /// ```
    #[must_use]
    pub fn with_timestamp_format(mut self, format: TimestampFormat) -> Self {
        self.timestamp_format = format;
        self
    }

    /// Set a custom timestamp format using a strftime-compatible format string
    #[must_use]
    pub fn with_custom_timestamp(mut self, format_str: &str) -> Self {
        self.timestamp_format = TimestampFormat::Custom(format_str.to_string());
        self
    }

    /// Check if rotation is needed based on the configured strategy
    fn should_rotate(&self) -> bool {
        match &self.policy.strategy {
            RotationStrategy::Never => false,

            RotationStrategy::Size { max_bytes } => self.current_size >= *max_bytes,

            RotationStrategy::Time { interval } => {
                let elapsed = SystemTime::now()
                    .duration_since(self.last_rotation)
                    .unwrap_or(Duration::ZERO);
                elapsed >= *interval
            }

            RotationStrategy::Daily { hour } => {
                let now: DateTime<Local> = SystemTime::now().into();
                let last: DateTime<Local> = self.last_rotation.into();

                // Rotate if we're on a different day and past the target hour
                now.date_naive() != last.date_naive() && now.hour() >= u32::from(*hour)
            }

            RotationStrategy::Hourly => {
                let elapsed = SystemTime::now()
                    .duration_since(self.last_rotation)
                    .unwrap_or(Duration::ZERO);
                elapsed >= Duration::from_secs(3600)
            }

            RotationStrategy::Hybrid { max_bytes, interval } => {
                let size_exceeded = self.current_size >= *max_bytes;
                let time_exceeded = SystemTime::now()
                    .duration_since(self.last_rotation)
                    .unwrap_or(Duration::ZERO)
                    >= *interval;
                size_exceeded || time_exceeded
            }
        }
    }

    /// Perform log rotation
    fn rotate(&mut self) -> Result<()> {
        // Flush and close current file
        // Explicitly drop writer to release file handle immediately
        if let Some(mut writer) = self.writer.take() {
            writer.flush().map_err(|e| {
                LoggerError::file_rotation(
                    self.base_path.display().to_string(),
                    format!("Failed to flush before rotation: {}", e),
                )
            })?;
            // Writer is dropped here, releasing file handle
        }

        // Delete oldest backup file that exceeds max_backup_files limit
        // This prevents unbounded disk usage from old rotated files
        let oldest_backup = self.backup_path(self.policy.max_backup_files);
        let oldest_compressed = oldest_backup.with_extension("log.gz");

        const MAX_DELETION_FAILURES: usize = 5;
        let mut deletion_failed = false;

        // Remove both compressed and uncompressed versions if they exist
        if oldest_compressed.exists() {
            if let Err(e) = fs::remove_file(&oldest_compressed) {
                deletion_failed = true;
                eprintln!(
                    "[WARN] Failed to remove oldest compressed backup {}: {} (failure #{}/{})",
                    oldest_compressed.display(),
                    e,
                    self.deletion_failure_count + 1,
                    MAX_DELETION_FAILURES
                );
            }
        }
        if oldest_backup.exists() {
            if let Err(e) = fs::remove_file(&oldest_backup) {
                deletion_failed = true;
                eprintln!(
                    "[WARN] Failed to remove oldest backup {}: {} (failure #{}/{})",
                    oldest_backup.display(),
                    e,
                    self.deletion_failure_count + 1,
                    MAX_DELETION_FAILURES
                );
            }
        }

        // Track deletion failures and abort rotation if threshold exceeded
        if deletion_failed {
            self.deletion_failure_count += 1;
            if self.deletion_failure_count >= MAX_DELETION_FAILURES {
                return Err(LoggerError::file_rotation(
                    self.base_path.display().to_string(),
                    format!(
                        "Rotation aborted: failed to delete old backup files {} consecutive times. \
                         This may indicate insufficient disk space or permission issues.",
                        self.deletion_failure_count
                    ),
                ));
            }
        } else {
            // Reset counter on successful deletion
            self.deletion_failure_count = 0;
        }

        // Rotate existing backup files
        for i in (1..self.policy.max_backup_files).rev() {
            let old_path = self.backup_path(i);
            let new_path = self.backup_path(i + 1);

            // Also check for compressed versions
            let old_compressed = old_path.with_extension("log.gz");
            let new_compressed = new_path.with_extension("log.gz");

            // Rotate compressed version if it exists
            if old_compressed.exists() {
                match fs::rename(&old_compressed, &new_compressed) {
                    Ok(_) => {},
                    Err(_) => {
                        if new_compressed.exists() {
                            let _ = fs::remove_file(&new_compressed);
                        }
                        let _ = fs::rename(&old_compressed, &new_compressed);
                    }
                }
            }
            // Rotate uncompressed version if it exists
            else if old_path.exists() {
                // Use rename which atomically replaces destination if it exists
                // This avoids TOCTOU issues with check-then-delete pattern
                match fs::rename(&old_path, &new_path) {
                    Ok(_) => {},
                    Err(_) => {
                        // On some platforms, rename fails if destination exists
                        // Try the remove-then-rename fallback
                        if new_path.exists() {
                            // Best effort remove - ignore errors if file was deleted by another process
                            let _ = fs::remove_file(&new_path);
                        }
                        // Retry rename
                        fs::rename(&old_path, &new_path).map_err(|e| {
                            LoggerError::file_rotation(
                                old_path.display().to_string(),
                                format!("Failed to rotate backup files: {}", e),
                            )
                        })?;
                    }
                }
            }
        }

        // Move current file to .1
        let backup_path = self.backup_path(1);
        if self.base_path.exists() {
            fs::rename(&self.base_path, &backup_path).map_err(|e| {
                LoggerError::file_rotation(
                    self.base_path.display().to_string(),
                    format!("Failed to rotate current log file: {}", e),
                )
            })?;

            // Compress if enabled
            if self.policy.compress {
                self.compress_file(&backup_path)?;
            }
        }

        // Open new file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.base_path)
            .map_err(|e| {
                LoggerError::file_rotation(
                    self.base_path.display().to_string(),
                    format!("Failed to create new log file: {}", e),
                )
            })?;

        self.writer = Some(BufWriter::new(file));
        self.current_size = 0;
        self.last_rotation = SystemTime::now();

        Ok(())
    }

    /// Get backup file path for given index
    fn backup_path(&self, index: usize) -> PathBuf {
        let mut path = self.base_path.clone();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app.log");
        path.set_file_name(format!("{}.{}", filename, index));
        path
    }

    /// Compress a log file with transactional safety using streaming I/O
    ///
    /// This method ensures the original file is only deleted after
    /// compression is fully successful, preventing data loss.
    ///
    /// Uses streaming compression to avoid loading entire file into memory,
    /// which is critical for large log files.
    fn compress_file(&self, path: &Path) -> Result<()> {
        use std::io::{BufReader, BufWriter, Read, Write};

        // Write compressed file to temporary location first
        let gz_path = path.with_extension("log.gz");
        let temp_gz_path = path.with_extension("log.gz.tmp");

        // Open input file with buffered reader for efficient streaming
        let input = File::open(path).map_err(|e| {
            LoggerError::io_operation(
                "compress log file",
                format!("Failed to open file for compression: {}", path.display()),
                e,
            )
        })?;
        let mut reader = BufReader::with_capacity(64 * 1024, input); // 64KB buffer

        // Create output file with buffered writer
        let output = File::create(&temp_gz_path).map_err(|e| {
            LoggerError::io_operation(
                "compress log file",
                format!("Failed to create temporary compressed file: {}", temp_gz_path.display()),
                e,
            )
        })?;
        let buffered_output = BufWriter::with_capacity(64 * 1024, output);

        // Create gzip encoder around buffered writer
        let mut encoder = flate2::write::GzEncoder::new(buffered_output, flate2::Compression::default());

        // Stream data from input to compressed output in chunks
        // This avoids loading the entire file into memory
        let mut buffer = vec![0u8; 64 * 1024]; // 64KB chunk size
        loop {
            let bytes_read = reader.read(&mut buffer).map_err(|e| {
                // Clean up temp file on read failure
                let _ = fs::remove_file(&temp_gz_path);
                LoggerError::io_operation(
                    "compress log file",
                    format!("Failed to read from file: {}", path.display()),
                    e,
                )
            })?;

            if bytes_read == 0 {
                break; // EOF reached
            }

            encoder.write_all(&buffer[..bytes_read]).map_err(|e| {
                // Clean up temp file on write failure
                let _ = fs::remove_file(&temp_gz_path);
                LoggerError::io_operation(
                    "compress log file",
                    "Failed to compress data chunk".to_string(),
                    e,
                )
            })?;
        }

        // Finish compression and explicitly finish encoder to ensure flush
        encoder.finish().map_err(|e| {
            // Clean up temp file on finish failure
            let _ = fs::remove_file(&temp_gz_path);
            LoggerError::io_operation(
                "compress log file",
                "Failed to finish compression".to_string(),
                e,
            )
        })?;

        // Atomically move temp file to final location
        // Only after successful compression do we replace any existing compressed file
        fs::rename(&temp_gz_path, &gz_path).map_err(|e| {
            // Clean up temp file on rename failure
            let _ = fs::remove_file(&temp_gz_path);
            LoggerError::io_operation(
                "compress log file",
                format!("Failed to rename compressed file to: {}", gz_path.display()),
                e,
            )
        })?;

        // Only remove original file after compression is fully successful
        // This ensures we never lose data due to compression failures
        if let Err(e) = fs::remove_file(path) {
            eprintln!(
                "[WARN] Compression succeeded but failed to remove original file {}: {}. \
                Both compressed and uncompressed versions exist.",
                path.display(),
                e
            );
            // Don't return error - compression succeeded, original file remaining is not critical
            // The file will be cleaned up on next rotation
        }

        Ok(())
    }

    /// Get current file size
    #[must_use]
    pub fn current_size(&self) -> u64 {
        self.current_size
    }

    /// Get base path
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.base_path
    }

    /// Get rotation policy
    #[must_use]
    pub fn policy(&self) -> &RotationPolicy {
        &self.policy
    }

    /// Get the timestamp of the last rotation
    #[must_use]
    pub fn last_rotation(&self) -> SystemTime {
        self.last_rotation
    }

    /// Get the rotation strategy
    #[must_use]
    pub fn strategy(&self) -> &RotationStrategy {
        &self.policy.strategy
    }

    /// Try to reopen the log file (used for recovery after rotation failure)
    fn try_reopen_file(path: &Path) -> Result<(File, u64, SystemTime)> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| {
                LoggerError::file_appender(
                    path.display().to_string(),
                    format!("Failed to reopen after rotation failure: {}", e),
                )
            })?;

        let metadata = file.metadata()
            .map_err(|e| LoggerError::file_appender(
                path.display().to_string(),
                format!("Cannot access file metadata after reopen: {}", e)
            ))?;
        let size = metadata.len();
        let last_rotation = metadata.modified().unwrap_or_else(|_| SystemTime::now());
        Ok((file, size, last_rotation))
    }
}

impl Appender for RotatingFileAppender {
    fn name(&self) -> &str {
        "RotatingFileAppender"
    }

    fn append(&mut self, entry: &LogEntry) -> Result<()> {
        // Check if rotation is needed
        if self.should_rotate() {
            if let Err(e) = self.rotate() {
                // Log rotation failed - try to recover by continuing with current file
                // This prevents losing log messages due to rotation failures
                eprintln!(
                    "[WARN] Log rotation failed: {}. Continuing with current file.",
                    e
                );

                // Try to reopen the file if writer is missing
                if self.writer.is_none() {
                    match Self::try_reopen_file(&self.base_path) {
                        Ok((file, size, last_rotation)) => {
                            self.writer = Some(BufWriter::new(file));
                            self.current_size = size;
                            self.last_rotation = last_rotation;
                        }
                        Err(reopen_err) => {
                            eprintln!(
                                "[ERROR] Failed to reopen log file after rotation failure: {}",
                                reopen_err
                            );
                            return Err(e); // Original rotation error
                        }
                    }
                }

                // Reset size tracking to prevent infinite rotation attempts
                // Allow file to grow larger than limit in this error case
                self.current_size = 0;
            }
        }

        // Format and write entry
        let timestamp_str = self.timestamp_format.format(&entry.timestamp);
        let formatted = format!("[{}] [{}] {}\n", timestamp_str, entry.level, entry.message);

        let bytes_written = formatted.len() as u64;

        if let Some(ref mut writer) = self.writer {
            writer.write_all(formatted.as_bytes()).map_err(|e| {
                LoggerError::file_appender(
                    self.base_path.display().to_string(),
                    format!("Failed to write log entry: {}", e),
                )
            })?;
            self.current_size += bytes_written;
            Ok(())
        } else {
            Err(LoggerError::writer("Writer not initialized"))
        }
    }

    fn flush(&mut self) -> Result<()> {
        if let Some(ref mut writer) = self.writer {
            writer.flush().map_err(|e| {
                LoggerError::file_appender(
                    self.base_path.display().to_string(),
                    format!("Failed to flush: {}", e),
                )
            })?;
        }
        Ok(())
    }
}

impl Drop for RotatingFileAppender {
    fn drop(&mut self) {
        // Flush and explicitly drop writer to ensure file handle is released
        // This prevents resource leaks when the appender is dropped
        if let Some(mut writer) = self.writer.take() {
            // Best effort flush - ignore errors during drop
            let _ = writer.flush();
            // Writer is explicitly dropped here, releasing file handle immediately
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::log_level::LogLevel;
    use std::fs;
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn test_rotation_strategy_constructors() {
        // Size-based
        let strategy = RotationStrategy::size(1024);
        assert_eq!(strategy, RotationStrategy::Size { max_bytes: 1024 });

        // Time-based
        let strategy = RotationStrategy::time(Duration::from_secs(3600));
        assert_eq!(
            strategy,
            RotationStrategy::Time {
                interval: Duration::from_secs(3600)
            }
        );

        // Daily
        let strategy = RotationStrategy::daily(2);
        assert_eq!(strategy, RotationStrategy::Daily { hour: 2 });

        // Hourly
        let strategy = RotationStrategy::hourly();
        assert_eq!(strategy, RotationStrategy::Hourly);

        // Hybrid
        let strategy = RotationStrategy::hybrid(1024, Duration::from_secs(3600));
        assert_eq!(
            strategy,
            RotationStrategy::Hybrid {
                max_bytes: 1024,
                interval: Duration::from_secs(3600)
            }
        );

        // Never
        let strategy = RotationStrategy::never();
        assert_eq!(strategy, RotationStrategy::Never);
    }

    #[test]
    #[should_panic(expected = "Hour must be between 0 and 23")]
    fn test_daily_strategy_invalid_hour() {
        let _ = RotationStrategy::daily(24);
    }

    #[test]
    fn test_rotation_policy_builder() {
        let policy = RotationPolicy::new()
            .with_max_size(1024)
            .with_max_backups(3)
            .with_compression(true);

        assert_eq!(policy.max_file_size(), Some(1024));
        assert_eq!(policy.max_backup_files, 3);
        assert!(policy.compress);
        assert_eq!(
            policy.strategy,
            RotationStrategy::Size { max_bytes: 1024 }
        );
    }

    #[test]
    fn test_rotation_policy_with_strategy() {
        let policy = RotationPolicy::new()
            .with_strategy(RotationStrategy::Daily { hour: 0 })
            .with_max_backups(7)
            .with_compression(true);

        assert_eq!(policy.strategy, RotationStrategy::Daily { hour: 0 });
        assert_eq!(policy.max_backup_files, 7);
        assert!(policy.compress);
        assert_eq!(policy.max_file_size(), None);
    }

    #[test]
    fn test_rotation_policy_hybrid_strategy() {
        let policy = RotationPolicy::new()
            .with_strategy(RotationStrategy::Hybrid {
                max_bytes: 50 * 1024 * 1024,
                interval: Duration::from_secs(24 * 3600),
            })
            .with_max_backups(14);

        assert_eq!(policy.max_file_size(), Some(50 * 1024 * 1024));
        assert_eq!(policy.max_backup_files, 14);
    }

    #[test]
    fn test_rotating_appender_creation() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let appender = RotatingFileAppender::new(&log_path);
        assert!(appender.is_ok());

        let appender = appender.unwrap();
        assert_eq!(appender.path(), log_path);
        assert_eq!(appender.current_size(), 0);
    }

    #[test]
    fn test_log_rotation_size_based() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("rotation.log");

        // Create policy with small file size for testing
        let policy = RotationPolicy::new()
            .with_max_size(100) // 100 bytes
            .with_max_backups(3);

        let mut appender = RotatingFileAppender::with_policy(&log_path, policy).unwrap();

        // Write entries until rotation occurs
        for i in 0..20 {
            let entry = LogEntry::new(LogLevel::Info, format!("Test message number {}", i));
            appender.append(&entry).unwrap();
        }

        appender.flush().unwrap();

        // Check that backup files exist
        let backup1 = log_path.with_file_name("rotation.log.1");
        assert!(backup1.exists() || log_path.with_file_name("rotation.log.1.gz").exists());
    }

    #[test]
    fn test_log_rotation_time_based() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("time_rotation.log");

        // Create policy with very short time interval for testing
        let policy = RotationPolicy::new()
            .with_strategy(RotationStrategy::Time {
                interval: Duration::from_millis(50),
            })
            .with_max_backups(3);

        let mut appender = RotatingFileAppender::with_policy(&log_path, policy).unwrap();

        // Write initial entry
        let entry = LogEntry::new(LogLevel::Info, "Initial message".to_string());
        appender.append(&entry).unwrap();
        appender.flush().unwrap();

        // Wait for time interval to elapse
        thread::sleep(Duration::from_millis(60));

        // Write another entry - should trigger rotation
        let entry = LogEntry::new(LogLevel::Info, "After interval".to_string());
        appender.append(&entry).unwrap();
        appender.flush().unwrap();

        // Check that backup file exists
        let backup1 = log_path.with_file_name("time_rotation.log.1");
        assert!(backup1.exists() || log_path.with_file_name("time_rotation.log.1.gz").exists());
    }

    #[test]
    fn test_log_rotation_hybrid() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("hybrid_rotation.log");

        // Create policy with hybrid strategy
        let policy = RotationPolicy::new()
            .with_strategy(RotationStrategy::Hybrid {
                max_bytes: 100,
                interval: Duration::from_secs(3600), // Long interval, won't trigger
            })
            .with_max_backups(3);

        let mut appender = RotatingFileAppender::with_policy(&log_path, policy).unwrap();

        // Write entries until size-based rotation occurs
        for i in 0..20 {
            let entry = LogEntry::new(LogLevel::Info, format!("Test message number {}", i));
            appender.append(&entry).unwrap();
        }

        appender.flush().unwrap();

        // Check that backup file exists (triggered by size)
        let backup1 = log_path.with_file_name("hybrid_rotation.log.1");
        assert!(backup1.exists() || log_path.with_file_name("hybrid_rotation.log.1.gz").exists());
    }

    #[test]
    fn test_no_rotation_with_never_strategy() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("never_rotation.log");

        let policy = RotationPolicy::new()
            .with_strategy(RotationStrategy::Never)
            .with_max_backups(3);

        let mut appender = RotatingFileAppender::with_policy(&log_path, policy).unwrap();

        // Write many entries
        for i in 0..100 {
            let entry = LogEntry::new(LogLevel::Info, format!("Test message number {}", i));
            appender.append(&entry).unwrap();
        }

        appender.flush().unwrap();

        // Check that no backup files exist
        let backup1 = log_path.with_file_name("never_rotation.log.1");
        assert!(!backup1.exists());
        assert!(!log_path.with_file_name("never_rotation.log.1.gz").exists());
    }

    #[test]
    fn test_multiple_rotations() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("multi.log");

        let policy = RotationPolicy::new().with_max_size(50).with_max_backups(2);

        let mut appender = RotatingFileAppender::with_policy(&log_path, policy).unwrap();

        // Write enough to cause multiple rotations
        for i in 0..100 {
            let entry = LogEntry::new(LogLevel::Info, format!("Entry {}", i));
            appender.append(&entry).unwrap();
        }

        appender.flush().unwrap();

        // Should have at most max_backup_files + 1 (current file)
        let log_files = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_str().unwrap().starts_with("multi.log"))
            .count();

        assert!(log_files <= 3); // current + 2 backups
    }

    #[test]
    fn test_last_rotation_getter() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let appender = RotatingFileAppender::new(&log_path).unwrap();
        let last_rotation = appender.last_rotation();

        // Last rotation should be close to now (within a few seconds)
        let elapsed = SystemTime::now()
            .duration_since(last_rotation)
            .unwrap_or(Duration::ZERO);
        assert!(elapsed < Duration::from_secs(5));
    }

    #[test]
    fn test_strategy_getter() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let policy = RotationPolicy::new()
            .with_strategy(RotationStrategy::Hourly)
            .with_max_backups(24);

        let appender = RotatingFileAppender::with_policy(&log_path, policy).unwrap();

        assert_eq!(appender.strategy(), &RotationStrategy::Hourly);
    }

    #[test]
    fn test_default_strategy() {
        let default = RotationStrategy::default();
        assert_eq!(
            default,
            RotationStrategy::Size {
                max_bytes: 10 * 1024 * 1024
            }
        );
    }
}
