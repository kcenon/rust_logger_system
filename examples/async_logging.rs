//! Async logging example
//!
//! Demonstrates high-performance async logging with multi-threaded scenarios.
//!
//! Run with: cargo run --example async_logging

use rust_logger_system::prelude::*;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    println!("=== Rust Logger System - Async Logging Example ===\n");

    // Create async logger with buffer size of 1000
    let mut logger = Logger::with_async(1000);

    // Add appenders
    logger.add_appender(Box::new(ConsoleAppender::new()));
    logger.add_appender(Box::new(FileAppender::new("async_test.log")?));

    println!("1. High-performance async logging:");

    // Log many messages quickly
    for i in 0..100 {
        logger.info(format!("Message #{}", i));
    }

    println!("   Logged 100 messages asynchronously");

    // Multi-threaded logging
    println!("\n2. Multi-threaded logging:");

    let logger_clone = std::sync::Arc::new(logger);

    let mut handles = vec![];
    for thread_id in 0..5 {
        let logger = std::sync::Arc::clone(&logger_clone);
        let handle = thread::spawn(move || {
            for i in 0..20 {
                logger.info(format!("Thread {} - Message {}", thread_id, i));
                thread::sleep(Duration::from_millis(10));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("   5 threads logged 20 messages each");

    // Give async logger time to flush
    thread::sleep(Duration::from_millis(100));

    println!("\n=== Example completed successfully! ===");
    println!("Check 'async_test.log' for file output");

    Ok(())
}
