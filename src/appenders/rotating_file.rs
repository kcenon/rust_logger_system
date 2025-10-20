//! Rotating file appender with automatic log rotation

use crate::core::appender::Appender;
use crate::core::error::{LoggerError, Result};
use crate::core::log_entry::LogEntry;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// Configuration for rotating file appender
#[derive(Debug, Clone)]
pub struct RotationPolicy {
    /// Maximum size of a single log file in bytes
    pub max_file_size: u64,
    /// Maximum number of rotated files to keep
    pub max_backup_files: usize,
    /// Whether to compress rotated files
    pub compress: bool,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10 MB
            max_backup_files: 5,
            compress: false,
        }
    }
}

impl RotationPolicy {
    /// Create a new rotation policy
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum file size
    #[must_use = "builder methods return a new value and do not modify the original"]
    pub fn with_max_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
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
}

/// Rotating file appender
pub struct RotatingFileAppender {
    base_path: PathBuf,
    policy: RotationPolicy,
    writer: Option<BufWriter<File>>,
    current_size: u64,
    /// Counter for consecutive deletion failures (reset on successful deletion)
    deletion_failure_count: usize,
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

        let current_size = file.metadata()
            .map_err(|e| LoggerError::file_appender(
                base_path.display().to_string(),
                format!("Cannot access file metadata: {}", e)
            ))?
            .len();
        let writer = Some(BufWriter::new(file));

        Ok(Self {
            base_path,
            policy,
            writer,
            current_size,
            deletion_failure_count: 0,
        })
    }

    /// Check if rotation is needed
    fn should_rotate(&self) -> bool {
        self.current_size >= self.policy.max_file_size
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

    /// Try to reopen the log file (used for recovery after rotation failure)
    fn try_reopen_file(path: &Path) -> Result<(File, u64)> {
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

        let size = file.metadata()
            .map_err(|e| LoggerError::file_appender(
                path.display().to_string(),
                format!("Cannot access file metadata after reopen: {}", e)
            ))?
            .len();
        Ok((file, size))
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
                        Ok((file, size)) => {
                            self.writer = Some(BufWriter::new(file));
                            self.current_size = size;
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
        let formatted = format!(
            "[{}] [{}] {}\n",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
            entry.level,
            entry.message
        );

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
    use tempfile::tempdir;

    #[test]
    fn test_rotation_policy_builder() {
        let policy = RotationPolicy::new()
            .with_max_size(1024)
            .with_max_backups(3)
            .with_compression(true);

        assert_eq!(policy.max_file_size, 1024);
        assert_eq!(policy.max_backup_files, 3);
        assert!(policy.compress);
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
    fn test_log_rotation() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("rotation.log");

        // Create policy with small file size for testing
        let policy = RotationPolicy::new()
            .with_max_size(100) // 100 bytes
            .with_max_backups(3);

        let mut appender = RotatingFileAppender::with_policy(&log_path, policy).unwrap();

        // Write entries until rotation occurs
        for i in 0..20 {
            let entry = LogEntry::new(
                LogLevel::Info,
                format!("Test message number {}", i),
            );
            appender.append(&entry).unwrap();
        }

        appender.flush().unwrap();

        // Check that backup files exist
        let backup1 = log_path.with_file_name("rotation.log.1");
        assert!(backup1.exists() || log_path.with_file_name("rotation.log.1.gz").exists());
    }

    #[test]
    fn test_multiple_rotations() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("multi.log");

        let policy = RotationPolicy::new()
            .with_max_size(50)
            .with_max_backups(2);

        let mut appender = RotatingFileAppender::with_policy(&log_path, policy).unwrap();

        // Write enough to cause multiple rotations
        for i in 0..100 {
            let entry = LogEntry::new(
                LogLevel::Info,
                format!("Entry {}", i),
            );
            appender.append(&entry).unwrap();
        }

        appender.flush().unwrap();

        // Should have at most max_backup_files + 1 (current file)
        let log_files = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .unwrap()
                    .starts_with("multi.log")
            })
            .count();

        assert!(log_files <= 3); // current + 2 backups
    }
}
