//! Basic logger usage example
//!
//! Demonstrates synchronous logging with console appender and different log levels.
//!
//! Run with: cargo run --example basic_usage

use rust_logger_system::prelude::*;

fn main() -> Result<()> {
    println!("=== Rust Logger System - Basic Usage Example ===\n");

    // Create a synchronous logger
    let mut logger = Logger::new();

    // Add console appender
    logger.add_appender(Box::new(ConsoleAppender::new()));

    // Set minimum log level
    logger.set_min_level(LogLevel::Trace);

    // Log messages at different levels
    println!("1. Logging at different levels:");
    logger.trace("This is a trace message");
    logger.debug("This is a debug message");
    logger.info("This is an info message");
    logger.warn("This is a warning message");
    logger.error("This is an error message");
    logger.fatal("This is a fatal message");

    println!("\n2. Logging with different minimum levels:");

    // Change minimum level
    logger.set_min_level(LogLevel::Info);
    println!("   Minimum level set to INFO - trace and debug won't show:");
    logger.trace("Trace message (hidden)");
    logger.debug("Debug message (hidden)");
    logger.info("Info message (visible)");
    logger.warn("Warning message (visible)");

    println!("\n=== Example completed successfully! ===");

    Ok(())
}
