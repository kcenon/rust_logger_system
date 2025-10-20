//! File logging example
//!
//! Demonstrates logging to both console and file appenders simultaneously.
//!
//! Run with: cargo run --example file_logging

use rust_logger_system::prelude::*;

fn main() -> Result<()> {
    println!("=== Rust Logger System - File Logging Example ===\n");

    // Create logger
    let mut logger = Logger::new();

    // Add both console and file appenders
    logger.add_appender(Box::new(ConsoleAppender::new()));
    logger.add_appender(Box::new(FileAppender::new("application.log")?));

    println!("1. Logging to both console and file:");

    // Log various messages
    logger.info("Application started");
    logger.debug("Loading configuration...");
    logger.info("Configuration loaded successfully");
    logger.warn("Using default settings for some options");
    logger.info("Connecting to database...");
    logger.info("Database connection established");
    logger.error("Failed to load optional plugin");
    logger.info("Application initialization complete");

    println!("\n2. Performing some operations:");

    // Simulate application work
    for i in 1..=5 {
        logger.info(format!("Processing item {}/5", i));
        if i == 3 {
            logger.warn("Item 3 took longer than expected");
        }
    }

    logger.info("All operations completed");

    // Flush to ensure all logs are written
    logger.flush()?;

    println!("\n=== Example completed successfully! ===");
    println!("Check 'application.log' for the full log output");

    Ok(())
}
