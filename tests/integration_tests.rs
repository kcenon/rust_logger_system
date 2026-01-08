//! Integration tests for logger system
//!
//! These tests verify:
//! - Log injection prevention
//! - Async logging with backpressure
//! - Error tracking
//! - Structured logging
//! - Thread safety
//! - Timestamp format support

use rust_logger_system::appenders::file::FileAppender;
use rust_logger_system::appenders::json::JsonAppender;
use rust_logger_system::appenders::Appender;
use rust_logger_system::core::log_context::LogContext;
use rust_logger_system::core::log_level::LogLevel;
use rust_logger_system::core::logger::Logger;
use rust_logger_system::core::timestamp::TimestampFormat;
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
    // Test that when async buffer is full with Block policy, all messages are logged
    use rust_logger_system::OverflowPolicy;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("backpressure_test.log");

    // Small buffer with Block policy to ensure no messages are dropped
    let mut logger = Logger::builder()
        .async_mode(5)
        .overflow_policy(OverflowPolicy::Block) // Block ensures all messages are logged
        .build();
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

    // Verify all messages were logged (Block policy ensures no drops)
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 20, "All messages should be logged with Block policy");
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

    // Verify dropped logs are tracked
    let dropped_count = logger.dropped_count();
    assert_eq!(dropped_count, 5, "Should track all dropped logs");
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

#[test]
fn test_timestamp_format_iso8601() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("timestamp_iso8601.log");

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap())
        .expect("Failed to create appender")
        .with_timestamp_format(TimestampFormat::Iso8601);
    logger.add_appender(Box::new(appender));

    logger.info("Test ISO 8601 format");
    logger.flush().expect("Failed to flush");

    let content = fs::read_to_string(&log_file).expect("Failed to read log file");

    // ISO 8601 format: 2025-01-08T10:30:45.123Z
    assert!(content.contains('T'), "Should contain 'T' separator");
    assert!(content.contains('Z'), "Should end with 'Z' for UTC");
}

#[test]
fn test_timestamp_format_unix_millis() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("timestamp_unix.log");

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap())
        .expect("Failed to create appender")
        .with_timestamp_format(TimestampFormat::UnixMillis);
    logger.add_appender(Box::new(appender));

    logger.info("Test Unix millis format");
    logger.flush().expect("Failed to flush");

    let content = fs::read_to_string(&log_file).expect("Failed to read log file");

    // Unix millis should be a long number
    // Extract timestamp from format: [timestamp] [level] ...
    let timestamp_str = content
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .expect("Failed to extract timestamp");

    // Should be parseable as a number
    let timestamp: i64 = timestamp_str.parse().expect("Should be a valid number");
    assert!(timestamp > 1_000_000_000_000, "Should be Unix millis (13+ digits)");
}

#[test]
fn test_timestamp_format_custom() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("timestamp_custom.log");

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap())
        .expect("Failed to create appender")
        .with_custom_timestamp("%Y/%m/%d %H:%M");
    logger.add_appender(Box::new(appender));

    logger.info("Test custom format");
    logger.flush().expect("Failed to flush");

    let content = fs::read_to_string(&log_file).expect("Failed to read log file");

    // Custom format: YYYY/MM/DD HH:MM
    // Extract timestamp from format: [timestamp] [level] ...
    let timestamp_str = content
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .expect("Failed to extract timestamp");

    // Timestamp should contain date separators '/' and not 'T'
    assert!(
        timestamp_str.contains('/'),
        "Should contain date separators in timestamp"
    );
    assert!(
        !timestamp_str.contains('T'),
        "Timestamp should not have ISO 8601 'T' separator"
    );
}

#[test]
fn test_json_appender_timestamp_format() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("timestamp_json.jsonl");

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Info);

    // Test ISO 8601 format in JSON (should be a string)
    let appender = JsonAppender::new(log_file.to_str().unwrap())
        .expect("Failed to create appender")
        .with_timestamp_format(TimestampFormat::Iso8601);
    logger.add_appender(Box::new(appender));

    logger.info("Test JSON timestamp");
    logger.flush().expect("Failed to flush");

    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let json: serde_json::Value = serde_json::from_str(&content).expect("Invalid JSON");

    // Timestamp should be a string for ISO 8601
    assert!(json["timestamp"].is_string(), "Timestamp should be a string");
    let timestamp_str = json["timestamp"].as_str().unwrap();
    assert!(
        timestamp_str.ends_with('Z'),
        "ISO 8601 should end with 'Z'"
    );
}

#[test]
fn test_json_appender_unix_timestamp() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("timestamp_json_unix.jsonl");

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Info);

    // Test Unix millis format in JSON (should be a number)
    let appender = JsonAppender::new(log_file.to_str().unwrap())
        .expect("Failed to create appender")
        .with_timestamp_format(TimestampFormat::UnixMillis);
    logger.add_appender(Box::new(appender));

    logger.info("Test JSON Unix timestamp");
    logger.flush().expect("Failed to flush");

    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let json: serde_json::Value = serde_json::from_str(&content).expect("Invalid JSON");

    // Timestamp should be a number for Unix millis
    assert!(
        json["timestamp"].is_number(),
        "Timestamp should be a number"
    );
    let timestamp = json["timestamp"].as_i64().unwrap();
    assert!(
        timestamp > 1_000_000_000_000,
        "Should be Unix millis (13+ digits)"
    );
}

#[test]
fn test_multiple_appenders_different_formats() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file1 = temp_dir.path().join("format1.log");
    let log_file2 = temp_dir.path().join("format2.log");

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Info);

    // First appender with ISO 8601
    let appender1 = FileAppender::new(log_file1.to_str().unwrap())
        .expect("Failed to create appender")
        .with_timestamp_format(TimestampFormat::Iso8601);

    // Second appender with Unix millis
    let appender2 = FileAppender::new(log_file2.to_str().unwrap())
        .expect("Failed to create appender")
        .with_timestamp_format(TimestampFormat::UnixMillis);

    logger.add_appender(Box::new(appender1));
    logger.add_appender(Box::new(appender2));

    logger.info("Test multiple formats");
    logger.flush().expect("Failed to flush");

    // Verify first file has ISO 8601 format
    let content1 = fs::read_to_string(&log_file1).expect("Failed to read log file 1");
    assert!(
        content1.contains('T') && content1.contains('Z'),
        "First file should have ISO 8601 format"
    );

    // Verify second file has Unix millis format
    let content2 = fs::read_to_string(&log_file2).expect("Failed to read log file 2");
    let timestamp_str = content2
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .expect("Failed to extract timestamp");
    let _timestamp: i64 = timestamp_str
        .parse()
        .expect("Second file should have numeric timestamp");
}

// ============================================================================
// Log Sampling Tests
// ============================================================================

#[test]
fn test_sampling_basic() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("sampling_basic.log");

    let mut logger = Logger::builder()
        .sample_rate(0.5) // Sample 50%
        .build();
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Log many messages
    for i in 0..1000 {
        logger.info(format!("Message {}", i));
    }

    logger.flush().expect("Failed to flush");

    // Check sampling metrics
    let sampler = logger.sampler().expect("Sampler should be configured");
    let metrics = sampler.metrics();

    // Should have sampled approximately 50% (with some tolerance)
    let rate = sampler.effective_sample_rate();
    assert!(
        (0.40..=0.60).contains(&rate),
        "Expected ~50% sample rate, got {:.2}%",
        rate * 100.0
    );

    // Verify the log file has approximately 50% of messages
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let lines: Vec<&str> = content.lines().collect();
    let line_count = lines.len();

    assert!(
        (400..=600).contains(&line_count),
        "Expected ~500 log entries, got {}",
        line_count
    );

    // Total should equal sampled + dropped
    assert_eq!(
        metrics.sampled_count() + metrics.dropped_count(),
        metrics.total_count(),
        "Total should equal sampled + dropped"
    );
}

#[test]
fn test_sampling_always_sample_critical() {
    use rust_logger_system::SamplingConfig;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("sampling_critical.log");

    // Configure sampling to drop everything except Error/Fatal
    let config = SamplingConfig::new(0.0) // Drop all
        .with_always_sample(vec![LogLevel::Error, LogLevel::Fatal]);

    let mut logger = Logger::builder()
        .with_sampling(config)
        .build();
    logger.set_min_level(LogLevel::Debug);

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Log at different levels
    for _ in 0..100 {
        logger.debug("Debug message");
        logger.info("Info message");
        logger.warn("Warn message");
    }
    for _ in 0..10 {
        logger.error("Error message");
        logger.fatal("Fatal message");
    }

    logger.flush().expect("Failed to flush");

    // Verify only Error and Fatal messages were logged
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let lines: Vec<&str> = content.lines().collect();

    // Should have exactly 20 lines (10 error + 10 fatal)
    assert_eq!(
        lines.len(),
        20,
        "Should have only Error and Fatal messages, got {}",
        lines.len()
    );

    assert!(!content.contains("Debug message"));
    assert!(!content.contains("Info message"));
    assert!(!content.contains("Warn message"));
    assert!(content.contains("Error message"));
    assert!(content.contains("Fatal message"));
}

#[test]
fn test_sampling_category_rates() {
    use rust_logger_system::SamplingConfig;
    use rust_logger_system::LogContext;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("sampling_category.log");

    // Configure different rates for different categories
    let config = SamplingConfig::new(1.0) // Log all by default
        .with_category_rate("noisy", 0.0); // Drop all "noisy" category logs

    let mut logger = Logger::builder()
        .with_sampling(config)
        .build();
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Log with different categories
    for i in 0..50 {
        let normal_ctx = LogContext::new().with_field("category", "normal");
        let noisy_ctx = LogContext::new().with_field("category", "noisy");

        logger.log_with_context(LogLevel::Info, format!("Normal {}", i), normal_ctx);
        logger.log_with_context(LogLevel::Info, format!("Noisy {}", i), noisy_ctx);
    }

    logger.flush().expect("Failed to flush");

    // Verify only "normal" category messages were logged
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let lines: Vec<&str> = content.lines().collect();

    // Should have exactly 50 lines (only normal category)
    assert_eq!(
        lines.len(),
        50,
        "Should have only 'normal' category messages, got {}",
        lines.len()
    );

    assert!(content.contains("Normal"));
    assert!(!content.contains("Noisy"));
}

#[test]
fn test_sampling_with_async_logging() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("sampling_async.log");

    let mut logger = Logger::builder()
        .sample_rate(0.5)
        .async_mode(100)
        .build();
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Log many messages
    for i in 0..500 {
        logger.info(format!("Async message {}", i));
    }

    // Give async worker time to process
    std::thread::sleep(Duration::from_millis(300));
    logger.flush().expect("Failed to flush");

    // Check sampling metrics
    let sampler = logger.sampler().expect("Sampler should be configured");
    let rate = sampler.effective_sample_rate();

    assert!(
        (0.40..=0.60).contains(&rate),
        "Expected ~50% sample rate in async mode, got {:.2}%",
        rate * 100.0
    );
}

#[test]
fn test_sampling_no_sampler() {
    // Test that logger without sampling works normally
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("no_sampling.log");

    let mut logger = Logger::builder().build(); // No sampling configured
    logger.set_min_level(LogLevel::Info);

    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Log messages
    for i in 0..100 {
        logger.info(format!("Message {}", i));
    }

    logger.flush().expect("Failed to flush");

    // Verify sampler is None
    assert!(logger.sampler().is_none(), "Sampler should be None");

    // Verify all messages were logged
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 100, "All messages should be logged without sampling");
}
