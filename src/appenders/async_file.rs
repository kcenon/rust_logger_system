//! Async file appender for non-blocking log file writing
//!
//! Uses tokio::fs for fully asynchronous file I/O

#[cfg(feature = "async-appenders")]
use crate::core::{AsyncAppender, LogEntry, LoggerError, Result};
#[cfg(feature = "async-appenders")]
use async_trait::async_trait;
#[cfg(feature = "async-appenders")]
use tokio::fs::{File, OpenOptions};
#[cfg(feature = "async-appenders")]
use tokio::io::{AsyncWriteExt, BufWriter};
#[cfg(feature = "async-appenders")]
use std::path::{Path, PathBuf};

/// Async file appender for non-blocking file writes
///
/// # Important: Explicit Flush Required
///
/// **CRITICAL**: Always call `flush()` before dropping this appender to ensure
/// all buffered data is written to disk. Buffered data (up to `buffer_size` bytes)
/// will be LOST if the appender is dropped without flushing.
///
/// # Example
///
/// ```no_run
/// use rust_logger_system::appenders::AsyncFileAppender;
/// use rust_logger_system::core::{AsyncAppender, LogEntry, LogLevel};
/// use chrono::Utc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut appender = AsyncFileAppender::new("app.log").await?;
///
/// let entry = LogEntry {
///     level: LogLevel::Info,
///     message: "Hello async world!".to_string(),
///     timestamp: Utc::now(),
///     file: None,
///     line: None,
///     module_path: None,
///     thread_id: "main".to_string(),
///     thread_name: Some("main".to_string()),
///     context: None,
/// };
///
/// appender.append(&entry).await?;
///
/// // IMPORTANT: Always flush before dropping
/// appender.flush().await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-appenders")]
pub struct AsyncFileAppender {
    writer: BufWriter<File>,
    path: PathBuf,
    buffer_size: usize,
}

#[cfg(feature = "async-appenders")]
impl AsyncFileAppender {
    /// Default buffer size (64 KB)
    pub const DEFAULT_BUFFER_SIZE: usize = 64 * 1024;

    /// Create a new async file appender
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the log file
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be created or opened
    pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
        Self::with_buffer_size(path, Self::DEFAULT_BUFFER_SIZE).await
    }

    /// Create a new async file appender with custom buffer size
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the log file
    /// * `buffer_size` - Size of the write buffer in bytes
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be created or opened
    pub async fn with_buffer_size(path: impl AsRef<Path>, buffer_size: usize) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;

        let writer = BufWriter::with_capacity(buffer_size, file);

        Ok(Self {
            writer,
            path,
            buffer_size,
        })
    }

    /// Get the log file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the buffer size
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

#[cfg(feature = "async-appenders")]
#[async_trait]
impl AsyncAppender for AsyncFileAppender {
    async fn append(&mut self, entry: &LogEntry) -> Result<()> {
        // Format log entry
        let mut message = format!(
            "[{}] [{:5}] [{}] {}",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
            entry.level.to_str(),
            entry.thread_name.as_ref().unwrap_or(&entry.thread_id),
            entry.message
        );

        // Add source location if available
        if let (Some(file), Some(line)) = (&entry.file, entry.line) {
            message.push_str(&format!(" ({}:{})", file, line));
        }

        // Append context fields if present
        if let Some(ref context) = entry.context {
            message.push_str(" | ");
            message.push_str(&context.to_string());
        }

        message.push('\n');

        // Write asynchronously
        self.writer
            .write_all(message.as_bytes())
            .await
            .map_err(LoggerError::from)?;

        Ok(())
    }

    async fn flush(&mut self) -> Result<()> {
        self.writer.flush().await.map_err(LoggerError::from)?;
        Ok(())
    }

    fn name(&self) -> &str {
        "async_file"
    }
}

#[cfg(feature = "async-appenders")]
impl Drop for AsyncFileAppender {
    fn drop(&mut self) {
        // IMPORTANT: Cannot perform async flush in Drop (Drop is not async)
        //
        // SECURITY CONSIDERATION:
        // Buffered data in BufWriter will be lost if flush() is not called explicitly.
        // The underlying file descriptor will be closed, which triggers OS-level flush,
        // but BufWriter's internal buffer (up to buffer_size bytes) may be lost.
        //
        // BEST PRACTICE:
        // Always call flush() explicitly before dropping AsyncFileAppender:
        //   appender.flush().await?;
        //   drop(appender);  // or let it go out of scope
        //
        // This is documented in the AsyncFileAppender struct documentation.

        // Note: We intentionally do NOT print warnings here as Drop can be called
        // frequently in tests and normal operation where flush was already called.
        // The struct documentation clearly states the requirement to flush explicitly.
    }
}

#[cfg(all(test, feature = "async-appenders"))]
mod tests {
    use super::*;
    use crate::core::LogLevel;
    use chrono::Utc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_async_file_appender_creation() {
        let dir = tempdir().expect("Failed to create temp dir");
        let log_path = dir.path().join("test.log");

        let appender = AsyncFileAppender::new(&log_path)
            .await
            .expect("Failed to create appender");

        assert_eq!(appender.path(), log_path.as_path());
        assert_eq!(appender.buffer_size(), AsyncFileAppender::DEFAULT_BUFFER_SIZE);
    }

    #[tokio::test]
    async fn test_async_file_appender_write() {
        let dir = tempdir().expect("Failed to create temp dir");
        let log_path = dir.path().join("test.log");

        let mut appender = AsyncFileAppender::new(&log_path)
            .await
            .expect("Failed to create appender");

        let entry = LogEntry {
            level: LogLevel::Info,
            message: "Test message".to_string(),
            timestamp: Utc::now(),
            file: Some("test.rs".to_string()),
            line: Some(42),
            module_path: Some("test".to_string()),
            thread_id: "main".to_string(),
            thread_name: Some("main".to_string()),
            context: None,
        };

        appender.append(&entry).await.expect("Failed to append");
        appender.flush().await.expect("Failed to flush");

        // Read file and verify content
        let content = tokio::fs::read_to_string(&log_path)
            .await
            .expect("Failed to read log file");

        assert!(content.contains("Test message"));
        assert!(content.contains("INFO"));
        assert!(content.contains("test.rs:42"));
    }

    #[tokio::test]
    async fn test_async_file_appender_multiple_writes() {
        let dir = tempdir().expect("Failed to create temp dir");
        let log_path = dir.path().join("test.log");

        let mut appender = AsyncFileAppender::new(&log_path)
            .await
            .expect("Failed to create appender");

        for i in 0..10 {
            let entry = LogEntry {
                level: LogLevel::Info,
                message: format!("Message {}", i),
                timestamp: Utc::now(),
                file: None,
                line: None,
                module_path: None,
                thread_id: "main".to_string(),
                thread_name: Some("main".to_string()),
                context: None,
            };

            appender.append(&entry).await.expect("Failed to append");
        }

        appender.flush().await.expect("Failed to flush");

        // Read file and verify content
        let content = tokio::fs::read_to_string(&log_path)
            .await
            .expect("Failed to read log file");

        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 10);

        for i in 0..10 {
            assert!(content.contains(&format!("Message {}", i)));
        }
    }

    #[tokio::test]
    async fn test_async_file_appender_custom_buffer_size() {
        let dir = tempdir().expect("Failed to create temp dir");
        let log_path = dir.path().join("test.log");

        let appender = AsyncFileAppender::with_buffer_size(&log_path, 1024)
            .await
            .expect("Failed to create appender");

        assert_eq!(appender.buffer_size(), 1024);
    }
}
