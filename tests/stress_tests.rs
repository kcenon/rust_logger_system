//! Stress tests for priority-based log preservation
//!
//! These tests verify:
//! - Critical logs are never dropped under heavy load
//! - High priority logs are preserved with retry mechanism
//! - PriorityConfig settings work correctly
//! - Thread safety under concurrent high-volume logging

use rust_logger_system::appenders::file::FileAppender;
use rust_logger_system::core::log_level::LogLevel;
use rust_logger_system::core::logger::Logger;
use rust_logger_system::{OverflowPolicy, PriorityConfig};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

/// Test that critical logs (Error, Fatal) are never dropped under heavy load
#[test]
fn test_critical_logs_never_dropped() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("critical_stress.log");

    // Very small buffer to force overflow
    let mut logger = Logger::builder()
        .async_mode(5)
        .overflow_policy(OverflowPolicy::DropNewest)
        .priority_config(PriorityConfig {
            preserve_critical: true,
            preserve_high: false,
            block_on_critical: true,
            high_priority_retry_count: 0,
        })
        .build();

    logger.set_min_level(LogLevel::Trace);
    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Flood with low priority logs
    for i in 0..100 {
        logger.debug(format!("Debug message {}", i));
    }

    // Send critical logs - these MUST be preserved
    for i in 0..10 {
        logger.error(format!("Critical error {}", i));
    }

    // More low priority logs
    for i in 0..100 {
        logger.trace(format!("Trace message {}", i));
    }

    // Wait for processing
    std::thread::sleep(Duration::from_millis(500));
    drop(logger);

    // Verify all critical logs are present
    let content = std::fs::read_to_string(&log_file).expect("Failed to read log file");
    for i in 0..10 {
        assert!(
            content.contains(&format!("Critical error {}", i)),
            "Critical error {} was dropped!",
            i
        );
    }

    // Verify metrics show critical logs were preserved
    // (Logger is dropped, so we verify through file content)
    let error_count = content.matches("ERROR").count();
    assert!(error_count >= 10, "Expected at least 10 ERROR logs, got {}", error_count);
}

/// Test that high priority logs get retry attempts
#[test]
fn test_high_priority_retry_mechanism() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("high_priority_stress.log");

    // Small buffer with high priority preservation enabled
    let mut logger = Logger::builder()
        .async_mode(3)
        .overflow_policy(OverflowPolicy::DropNewest)
        .priority_config(PriorityConfig {
            preserve_critical: true,
            preserve_high: true,
            block_on_critical: true,
            high_priority_retry_count: 5,
        })
        .build();

    logger.set_min_level(LogLevel::Trace);
    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Send warnings interspersed with debug logs
    for i in 0..20 {
        logger.debug(format!("Debug {}", i));
        if i % 4 == 0 {
            logger.warn(format!("Warning {}", i / 4));
        }
    }

    // Wait for processing
    std::thread::sleep(Duration::from_millis(500));
    drop(logger);

    // Verify at least some warnings are preserved (retry should help)
    let content = std::fs::read_to_string(&log_file).expect("Failed to read log file");
    let warn_count = content.matches("WARN").count();

    // With retry mechanism, we expect more warnings to be preserved than without
    // This is a probabilistic test - at least some should be preserved
    assert!(warn_count > 0, "Expected some WARN logs to be preserved with retry");
}

/// Test PriorityConfig with preserve_critical disabled
#[test]
fn test_priority_config_preserve_critical_disabled() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("no_preserve.log");

    // Disable critical preservation
    let mut logger = Logger::builder()
        .async_mode(2)
        .overflow_policy(OverflowPolicy::DropNewest)
        .priority_config(PriorityConfig {
            preserve_critical: false,
            preserve_high: false,
            block_on_critical: false,
            high_priority_retry_count: 0,
        })
        .build();

    logger.set_min_level(LogLevel::Trace);
    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Flood the buffer
    for i in 0..50 {
        logger.debug(format!("Debug {}", i));
        logger.error(format!("Error {}", i));
    }

    std::thread::sleep(Duration::from_millis(300));
    drop(logger);

    // With preserve_critical disabled and DropNewest policy,
    // some errors may be dropped (this is expected behavior)
    let content = std::fs::read_to_string(&log_file).expect("Failed to read log file");
    let _error_count = content.matches("ERROR").count();

    // We can't guarantee exact counts, but verify the file was written
    assert!(!content.is_empty(), "Log file should not be empty");
    // The key point is this test doesn't panic/hang
}

/// Test concurrent logging with priority preservation
#[test]
fn test_concurrent_priority_logging() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("concurrent_priority.log");

    let logger = Arc::new({
        let mut l = Logger::builder()
            .async_mode(20)
            .overflow_policy(OverflowPolicy::AlertAndDrop)
            .priority_config(PriorityConfig::default())
            .build();
        l.set_min_level(LogLevel::Trace);
        let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
        l.add_appender(Box::new(appender));
        l
    });

    let critical_count = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    // Spawn multiple threads with different log levels
    for thread_id in 0..5 {
        let logger_clone = Arc::clone(&logger);
        let critical_clone = Arc::clone(&critical_count);

        let handle = std::thread::spawn(move || {
            for i in 0..20 {
                match thread_id % 3 {
                    0 => logger_clone.debug(format!("T{} Debug {}", thread_id, i)),
                    1 => logger_clone.warn(format!("T{} Warn {}", thread_id, i)),
                    2 => {
                        logger_clone.error(format!("T{} Error {}", thread_id, i));
                        critical_clone.fetch_add(1, Ordering::Relaxed);
                    }
                    _ => unreachable!(),
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Give time for async processing
    std::thread::sleep(Duration::from_millis(500));
    drop(logger);

    // Verify critical logs are preserved
    let content = std::fs::read_to_string(&log_file).expect("Failed to read log file");
    let actual_critical = content.matches("ERROR").count();
    let expected_critical = critical_count.load(Ordering::Relaxed);

    // All critical logs should be preserved
    assert_eq!(
        actual_critical, expected_critical,
        "Expected {} critical logs, got {}",
        expected_critical, actual_critical
    );
}

/// Test that block_on_critical setting works
#[test]
fn test_block_on_critical_setting() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("block_critical.log");

    // Test with block_on_critical = true
    let mut logger = Logger::builder()
        .async_mode(1)
        .overflow_policy(OverflowPolicy::DropNewest)
        .priority_config(PriorityConfig {
            preserve_critical: true,
            preserve_high: false,
            block_on_critical: true,
            high_priority_retry_count: 0,
        })
        .build();

    logger.set_min_level(LogLevel::Error);
    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Send critical logs - should block to ensure delivery
    for i in 0..5 {
        logger.fatal(format!("Fatal {}", i));
    }

    std::thread::sleep(Duration::from_millis(300));
    drop(logger);

    // All fatal logs should be written
    let content = std::fs::read_to_string(&log_file).expect("Failed to read log file");
    for i in 0..5 {
        assert!(
            content.contains(&format!("Fatal {}", i)),
            "Fatal {} was not written with block_on_critical=true",
            i
        );
    }
}

/// Test metrics tracking for priority preservation
#[test]
fn test_priority_metrics_tracking() {
    let logger = Logger::builder()
        .async_mode(2)
        .overflow_policy(OverflowPolicy::DropNewest)
        .priority_config(PriorityConfig::default())
        .build();

    // Generate overflow with mixed priorities
    for _ in 0..50 {
        logger.debug("Debug log");
    }

    // Critical logs should be preserved
    for _ in 0..5 {
        logger.error("Critical log");
    }

    std::thread::sleep(Duration::from_millis(200));

    let metrics = logger.metrics();

    // Queue full events should have occurred
    assert!(
        metrics.queue_full_events() > 0,
        "Expected queue full events due to small buffer"
    );

    // Critical logs should be preserved
    assert!(
        metrics.critical_logs_preserved() > 0,
        "Expected critical logs to be preserved"
    );
}

/// Stress test with rapid log bursts
#[test]
fn test_rapid_burst_logging() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("burst.log");

    let mut logger = Logger::builder()
        .async_mode(10)
        .overflow_policy(OverflowPolicy::AlertAndDrop)
        .priority_config(PriorityConfig::default())
        .build();

    logger.set_min_level(LogLevel::Trace);
    let appender = FileAppender::new(log_file.to_str().unwrap()).expect("Failed to create appender");
    logger.add_appender(Box::new(appender));

    // Rapid bursts with critical log at the end
    for burst in 0..10 {
        for i in 0..20 {
            logger.trace(format!("Burst {} trace {}", burst, i));
        }
        // Critical log after each burst
        logger.fatal(format!("Burst {} complete", burst));
    }

    std::thread::sleep(Duration::from_millis(500));
    drop(logger);

    // Verify all burst completion markers are present
    let content = std::fs::read_to_string(&log_file).expect("Failed to read log file");
    for burst in 0..10 {
        assert!(
            content.contains(&format!("Burst {} complete", burst)),
            "Burst {} completion marker missing!",
            burst
        );
    }
}
