# Rust Logger System Configuration Guide

> **Languages**: English | [한국어](./CONFIGURATION.ko.md)

## Overview

This guide covers all configuration options for the Rust Logger System, including logger modes, log levels, appenders, and advanced configuration scenarios.

## Table of Contents

1. [Logger Modes](#logger-modes)
2. [Log Levels](#log-levels)
3. [Appenders](#appenders)
4. [Configuration Patterns](#configuration-patterns)
5. [Environment-Specific Configuration](#environment-specific-configuration)
6. [Performance Tuning](#performance-tuning)
7. [Troubleshooting](#troubleshooting)

## Logger Modes

### Synchronous Logger

**Use when:**
- Simple applications
- Low logging volume
- Immediate log output required
- Debugging

**Example:**
```rust
use rust_logger_system::prelude::*;

let mut logger = Logger::new();
logger.add_appender(Box::new(ConsoleAppender::new()));
logger.set_min_level(LogLevel::Info);
```

**Characteristics:**
- Logs written immediately
- Blocks calling thread during I/O
- Simple and predictable
- Lower throughput

### Asynchronous Logger

**Use when:**
- High-performance applications
- High logging volume
- Cannot afford I/O blocking
- Production environments

**Example:**
```rust
use rust_logger_system::prelude::*;

let mut logger = Logger::with_async(10000); // Buffer size: 10000
logger.add_appender(Box::new(FileAppender::new("app.log")?));
logger.set_min_level(LogLevel::Info);
```

**Characteristics:**
- Non-blocking log operations
- Background worker thread
- Bounded channel buffer
- Higher throughput

**Buffer Size Guidelines:**
- Small apps (< 100 logs/sec): 1,000
- Medium apps (100-1000 logs/sec): 10,000
- Large apps (> 1000 logs/sec): 100,000

## Log Levels

### Level Hierarchy

```
Trace < Debug < Info < Warn < Error < Fatal
```

### Level Definitions

| Level | Value | Use Case | Examples |
|-------|-------|----------|----------|
| **Trace** | 0 | Very detailed debugging | Function entry/exit, variable values |
| **Debug** | 1 | Development debugging | State changes, internal operations |
| **Info** | 2 | General information | Service started, configuration loaded |
| **Warn** | 3 | Warning conditions | Deprecated API usage, config defaults |
| **Error** | 4 | Error conditions | Failed operations, exceptions |
| **Fatal** | 5 | Critical failures | System crash, data corruption |

### Setting Minimum Level

```rust
// Only Info and above will be logged
logger.set_min_level(LogLevel::Info);

logger.trace("Not logged");  // Ignored
logger.debug("Not logged");  // Ignored
logger.info("Logged");       // Written
logger.warn("Logged");       // Written
logger.error("Logged");      // Written
```

### Dynamic Level Changes

```rust
// Start with Info level
logger.set_min_level(LogLevel::Info);

// Enable debug logging dynamically
logger.set_min_level(LogLevel::Debug);

// Disable all logging
logger.set_min_level(LogLevel::Fatal);
```

### Level Configuration by Environment

```rust
use std::env;

let log_level = match env::var("LOG_LEVEL").as_deref() {
    Ok("trace") => LogLevel::Trace,
    Ok("debug") => LogLevel::Debug,
    Ok("warn") => LogLevel::Warn,
    Ok("error") => LogLevel::Error,
    _ => LogLevel::Info, // Default
};

logger.set_min_level(log_level);
```

## Appenders

### Console Appender

Outputs logs to stdout with ANSI color support.

```rust
use rust_logger_system::prelude::*;

let console = ConsoleAppender::new();
logger.add_appender(Box::new(console));
```

**Configuration:**
- Automatic ANSI color support
- UTF-8 encoding
- Unbuffered output (immediate visibility)

**Color Scheme:**
- Trace: Gray
- Debug: Cyan
- Info: Green
- Warn: Yellow
- Error: Red
- Fatal: Bright Red (bold)

### File Appender

Writes logs to files.

```rust
use rust_logger_system::prelude::*;

let file = FileAppender::new("application.log")?;
logger.add_appender(Box::new(file));
```

**Configuration Options:**

#### Basic File Logging
```rust
let file = FileAppender::new("app.log")?;
```

#### Custom Path
```rust
use std::path::PathBuf;

let log_dir = PathBuf::from("/var/log/myapp");
std::fs::create_dir_all(&log_dir)?;
let file = FileAppender::new(log_dir.join("app.log"))?;
```

#### Multiple Files
```rust
// General logs
logger.add_appender(Box::new(FileAppender::new("app.log")?));

// Error-only logs (filter in application code)
logger.add_appender(Box::new(FileAppender::new("errors.log")?));
```

**File Format:**
```
[2025-10-16 10:30:45.123] [INFO] Application started
[2025-10-16 10:30:45.456] [DEBUG] Configuration loaded: config.toml
[2025-10-16 10:30:46.789] [ERROR] Failed to connect: Connection refused
```

### Multiple Appenders

```rust
// Log to both console and file
logger.add_appender(Box::new(ConsoleAppender::new()));
logger.add_appender(Box::new(FileAppender::new("app.log")?));

// All logs go to both destinations
logger.info("This appears in console AND file");
```

### Custom Appenders

Implement the `Appender` trait:

```rust
use rust_logger_system::prelude::*;

struct NetworkAppender {
    endpoint: String,
}

impl Appender for NetworkAppender {
    fn append(&mut self, entry: &LogEntry) -> Result<()> {
        // Send log to remote server
        let json = serde_json::json!({
            "timestamp": entry.timestamp,
            "level": format!("{:?}", entry.level),
            "message": entry.message,
        });

        // Send via HTTP, etc.
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        // Ensure all logs sent
        Ok(())
    }
}

// Use custom appender
logger.add_appender(Box::new(NetworkAppender {
    endpoint: "https://logs.example.com/ingest".to_string(),
}));
```

## Configuration Patterns

### Development Configuration

```rust
use rust_logger_system::prelude::*;

fn create_dev_logger() -> Logger {
    let mut logger = Logger::new(); // Synchronous for debugging

    logger.add_appender(Box::new(ConsoleAppender::new()));
    logger.set_min_level(LogLevel::Debug); // Verbose logging

    logger
}
```

### Production Configuration

```rust
use rust_logger_system::prelude::*;

fn create_prod_logger() -> Result<Logger> {
    let mut logger = Logger::with_async(10000); // High-performance async

    // Console for container logs
    logger.add_appender(Box::new(ConsoleAppender::new()));

    // File for persistent logs
    logger.add_appender(Box::new(FileAppender::new("/var/log/app/app.log")?));

    logger.set_min_level(LogLevel::Info); // Production level

    Ok(logger)
}
```

### Testing Configuration

```rust
use rust_logger_system::prelude::*;

fn create_test_logger() -> Logger {
    let mut logger = Logger::new();

    // No console output in tests
    logger.set_min_level(LogLevel::Warn); // Only warnings and errors

    logger
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_logging() {
        let logger = create_test_logger();

        // Test logic with minimal logging
    }
}
```

## Environment-Specific Configuration

### Using Environment Variables

```rust
use std::env;
use rust_logger_system::prelude::*;

fn create_logger_from_env() -> Result<Logger> {
    // Read configuration from environment
    let log_level = env::var("LOG_LEVEL")
        .unwrap_or_else(|_| "info".to_string());

    let log_file = env::var("LOG_FILE")
        .unwrap_or_else(|_| "app.log".to_string());

    let async_mode = env::var("LOG_ASYNC")
        .map(|v| v == "true")
        .unwrap_or(true);

    // Create logger based on environment
    let mut logger = if async_mode {
        Logger::with_async(10000)
    } else {
        Logger::new()
    };

    // Set level
    let level = match log_level.to_lowercase().as_str() {
        "trace" => LogLevel::Trace,
        "debug" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "warn" => LogLevel::Warn,
        "error" => LogLevel::Error,
        "fatal" => LogLevel::Fatal,
        _ => LogLevel::Info,
    };
    logger.set_min_level(level);

    // Add appenders
    logger.add_appender(Box::new(ConsoleAppender::new()));

    if !log_file.is_empty() {
        logger.add_appender(Box::new(FileAppender::new(&log_file)?));
    }

    Ok(logger)
}
```

### Configuration File Example

```toml
# config.toml
[logging]
level = "info"
async = true
buffer_size = 10000

[[logging.appenders]]
type = "console"

[[logging.appenders]]
type = "file"
path = "/var/log/app/app.log"
```

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct LoggingConfig {
    level: String,
    async_mode: bool,
    buffer_size: usize,
    appenders: Vec<AppenderConfig>,
}

#[derive(Deserialize)]
struct AppenderConfig {
    #[serde(rename = "type")]
    appender_type: String,
    path: Option<String>,
}

fn create_logger_from_config(config: LoggingConfig) -> Result<Logger> {
    let mut logger = if config.async_mode {
        Logger::with_async(config.buffer_size)
    } else {
        Logger::new()
    };

    // Set level from config
    let level = match config.level.as_str() {
        "trace" => LogLevel::Trace,
        "debug" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "warn" => LogLevel::Warn,
        "error" => LogLevel::Error,
        "fatal" => LogLevel::Fatal,
        _ => LogLevel::Info,
    };
    logger.set_min_level(level);

    // Add appenders from config
    for appender_cfg in config.appenders {
        match appender_cfg.appender_type.as_str() {
            "console" => {
                logger.add_appender(Box::new(ConsoleAppender::new()));
            }
            "file" => {
                if let Some(path) = appender_cfg.path {
                    logger.add_appender(Box::new(FileAppender::new(&path)?));
                }
            }
            _ => {}
        }
    }

    Ok(logger)
}
```

## Performance Tuning

### Async Buffer Sizing

**Too Small:**
- Risk of blocking when buffer fills
- Loss of async benefits

**Too Large:**
- Excessive memory usage
- Potential message loss on crash

**Recommended Sizing:**
```rust
// Calculate based on expected load
let logs_per_second = 1000;
let burst_multiplier = 10; // Handle 10x burst
let buffer_size = logs_per_second * burst_multiplier;

let logger = Logger::with_async(buffer_size);
```

### Minimizing Allocations

```rust
// Prefer format! for complex messages
logger.info(format!("User {} logged in from {}", user_id, ip));

// For simple messages, use string literals
logger.info("Server started");

// Avoid unnecessary string operations
// Bad: Creates intermediate strings
logger.info(format!("{}", format!("User: {}", user)));

// Good: Single format operation
logger.info(format!("User: {}", user));
```

### Conditional Logging

```rust
// Expensive operations only when level is enabled
if logger.min_level() <= LogLevel::Debug {
    let debug_info = compute_expensive_debug_info();
    logger.debug(format!("Debug info: {}", debug_info));
}
```

## Troubleshooting

### Logs Not Appearing

**Problem:** Logs not showing up in output

**Solutions:**

1. Check minimum log level
```rust
// Ensure level is low enough
logger.set_min_level(LogLevel::Trace); // Temporarily enable all
```

2. Ensure appenders are added
```rust
// Verify appenders exist
if logger.appender_count() == 0 {
    logger.add_appender(Box::new(ConsoleAppender::new()));
}
```

3. Flush async logger
```rust
// For async logger, ensure flush
logger.flush()?;
```

### Async Logger Blocking

**Problem:** Async logger blocking despite async mode

**Cause:** Buffer full

**Solutions:**

1. Increase buffer size
```rust
let logger = Logger::with_async(100000); // Larger buffer
```

2. Reduce logging volume
```rust
// Increase minimum level
logger.set_min_level(LogLevel::Warn);
```

3. Check for slow appenders
```rust
// Remove slow appenders or optimize them
```

### File Permission Issues

**Problem:** Cannot write to log file

**Solutions:**

1. Check directory permissions
```rust
use std::fs;

let log_dir = "/var/log/myapp";
fs::create_dir_all(log_dir)?; // Create if doesn't exist
```

2. Use writable location
```rust
// Use user directory or temp
use std::env;

let log_path = env::temp_dir().join("app.log");
let file = FileAppender::new(log_path)?;
```

### Memory Usage

**Problem:** High memory usage with async logger

**Solutions:**

1. Reduce buffer size
```rust
let logger = Logger::with_async(1000); // Smaller buffer
```

2. Reduce message sizes
```rust
// Truncate long messages
let message = long_string[..500].to_string();
logger.info(message);
```

### Thread Safety Issues

**Problem:** Concurrent access errors

**Solution:** Use Arc for shared logger
```rust
use std::sync::Arc;

let logger = Arc::new(Logger::with_async(10000));

let logger_clone = Arc::clone(&logger);
std::thread::spawn(move || {
    logger_clone.info("Thread-safe logging");
});
```

## Best Practices

### General Guidelines

1. **Use async logger in production**
   - Better performance
   - Non-blocking I/O

2. **Set appropriate log levels**
   - Development: Debug
   - Production: Info
   - Troubleshooting: Trace

3. **Include context in messages**
   ```rust
   logger.error(format!("Failed to process order {}: {}", order_id, error));
   ```

4. **Flush on shutdown**
   ```rust
   // Before application exit
   logger.flush()?;
   ```

5. **Use structured logging for important data**
   ```rust
   logger.info(format!("order_id={} user_id={} amount={}",
       order_id, user_id, amount));
   ```

---

*Configuration Guide Version 1.0*
*Last Updated: 2025-10-16*
