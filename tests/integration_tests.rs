//! Integration tests for logger system
//!
//! These tests verify:
//! - Log injection prevention
//! - Async logging with backpressure
//! - Error tracking
//! - Structured logging
//! - Thread safety

use rust_logger_system::appenders::file::FileAppender;
use rust_logger_system::appenders::Appender;
use rust_logger_system::core::log_context::LogContext;
use rust_logger_system::core::log_level::LogLevel;
use rust_logger_system::core::logger::Logger;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_log_injection_prevention() {
    // Test that newlines are escaped to prevent log injection attacks
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("injection_test.log");

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Try to inject fake log entries with newlines
    let malicious_message = "User login\nERROR [2024-10-17] Fake error injected\nINFO Continuation";
    logger.info(malicious_message);

    logger.flush().expect("Failed to flush");

    // Read log file
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");

    // Verify newlines are escaped
    assert!(content.contains("\\n"));
    assert!(!content.contains("\nERROR [2024-10-17] Fake error injected\n"));

    // The malicious message should appear as a single line with escaped newlines
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1, "Log should be a single line, not multiple");
}

#[test]
fn test_async_logging() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("async_test.log");

    let mut logger = Logger::with_async(100);
    logger.set_min_level(LogLevel::Debug);

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Log many messages
    for i in 0..50 {
        logger.info(format!("Message {}", i));
    }

    // Give async worker time to process
    std::thread::sleep(Duration::from_millis(200));

    logger.flush().expect("Failed to flush");

    // Verify all messages were logged
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 50, "Should have 50 log entries");
}

#[test]
fn test_async_backpressure() {
    // Test that when async buffer is full, logger falls back to sync logging
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("backpressure_test.log");

    // Small buffer to trigger backpressure
    let mut logger = Logger::with_async(5);
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Log many messages quickly to exceed buffer
    for i in 0..20 {
        logger.info(format!("Message {}", i));
    }

    // Give time to process
    std::thread::sleep(Duration::from_millis(300));

    logger.flush().expect("Failed to flush");

    // Verify all messages were logged (either async or sync fallback)
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 20, "All messages should be logged despite backpressure");
}

#[test]
fn test_error_tracking() {
    // Test that failed log writes are tracked
    struct FailingAppender {
        fail_count: std::sync::atomic::AtomicUsize,
    }

    impl Appender for FailingAppender {
        fn append(&mut self, _entry: &rust_logger_system::core::log_entry::LogEntry) -> rust_logger_system::core::error::Result<()> {
            self.fail_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Err(rust_logger_system::core::error::LoggerError::other("Simulated failure"))
        }

        fn flush(&mut self) -> rust_logger_system::core::error::Result<()> {
            Ok(())
        }

        fn name(&self) -> &str {
            "FailingAppender"
        }
    }

    let mut logger = Logger::new();
    logger.add_appender(Box::new(FailingAppender {
        fail_count: std::sync::atomic::AtomicUsize::new(0),
    }));

    // Log some messages
    for _ in 0..5 {
        logger.info("Test message");
    }

    // Verify failed writes are tracked
    let failed_count = logger.failed_write_count();
    assert_eq!(failed_count, 5, "Should track all failed writes");
}

#[test]
fn test_structured_logging() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("structured_test.log");

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Log with context
    let mut context = LogContext::new();
    context.add_field("user_id", "12345");
    context.add_field("request_id", "abc-def-ghi");
    context.add_field("ip_address", "192.168.1.1");

    logger.info_with_context("User logged in", context);

    logger.flush().expect("Failed to flush");

    // Verify context is in the log
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    assert!(content.contains("user_id"));
    assert!(content.contains("12345"));
    assert!(content.contains("request_id"));
}

#[test]
fn test_concurrent_logging() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("concurrent_test.log");

    let mut logger = Logger::with_async(200);
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    let logger = Arc::new(logger);

    // Spawn multiple threads logging concurrently
    let mut handles = vec![];
    for thread_id in 0..5 {
        let logger_clone = Arc::clone(&logger);
        let handle = std::thread::spawn(move || {
            for i in 0..10 {
                logger_clone.info(format!("Thread {} - Message {}", thread_id, i));
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Give async worker time to process
    std::thread::sleep(Duration::from_millis(300));

    logger.flush().expect("Failed to flush");

    // Verify all messages were logged
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 50, "Should have 50 log entries from 5 threads * 10 messages");
}

#[test]
fn test_log_levels() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("levels_test.log");

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Warn); // Only warn and above

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Log at different levels
    logger.trace("Trace message");
    logger.debug("Debug message");
    logger.info("Info message");
    logger.warn("Warn message");
    logger.error("Error message");
    logger.fatal("Fatal message");

    logger.flush().expect("Failed to flush");

    // Verify only warn and above are logged
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    assert!(!content.contains("Trace message"));
    assert!(!content.contains("Debug message"));
    assert!(!content.contains("Info message"));
    assert!(content.contains("Warn message"));
    assert!(content.contains("Error message"));
    assert!(content.contains("Fatal message"));
}

#[test]
fn test_multiple_appenders() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file1 = temp_dir.path().join("multi1.log");
    let log_file2 = temp_dir.path().join("multi2.log");

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Info);

    // Add two file appenders
    let appender1 = FileAppender::new(log_file1.to_str().unwrap()).expect("Failed to create appender");
    let appender2 = FileAppender::new(log_file2.to_str().unwrap()).expect("Failed to create appender");

    logger.add_appender(Box::new(appender1));
    logger.add_appender(Box::new(appender2));

    logger.info("Test message");

    logger.flush().expect("Failed to flush");

    // Verify both files have the message
    let content1 = fs::read_to_string(&log_file1).expect("Failed to read log file 1");
    let content2 = fs::read_to_string(&log_file2).expect("Failed to read log file 2");

    assert!(content1.contains("Test message"));
    assert!(content2.contains("Test message"));
}

#[test]
fn test_special_characters_escaping() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("special_chars_test.log");

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Log message with special characters
    logger.info("Message with\ttab\rand\ncarriage return");

    logger.flush().expect("Failed to flush");

    let content = fs::read_to_string(&log_file).expect("Failed to read log file");

    // Verify special characters are escaped
    assert!(content.contains("\\t"));
    assert!(content.contains("\\r"));
    assert!(content.contains("\\n"));
}

#[test]
fn test_graceful_shutdown() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("shutdown_test.log");

    {
        let mut logger = Logger::with_async(100);
        logger.set_min_level(LogLevel::Info);

        let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
        logger.add_appender(Box::new(appender));

        // Log messages
        for i in 0..10 {
            logger.info(format!("Message {}", i));
        }

        // Logger drops here - should flush and shutdown gracefully
    }

    // Give time for cleanup
    std::thread::sleep(Duration::from_millis(200));

    // Verify all messages were written
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 10, "All messages should be written before shutdown");
}
