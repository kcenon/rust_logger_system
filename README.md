# Rust Logger System

[English](README.md) | [한국어](README.ko.md)

A production-ready, high-performance Rust logging framework designed to provide comprehensive logging capabilities with asynchronous processing, multiple output targets, and minimal overhead.

This is a Rust implementation of the [logger_system](https://github.com/kcenon/logger_system) project, providing the same functionality with Rust's safety guarantees and performance benefits.

## Quality Status

- Verification: `cargo check`, `cargo test` (unit, integration, property, doc), `cargo clippy --all-targets` 모두 통과 ✅
- Known issues: 없음. 단, `AsyncFileAppender` 사용 시 `flush()`를 호출해야 버퍼 손실을 방지할 수 있습니다.
- Production guidance: 기본 설정 그대로 프로덕션 투입 가능. 장기 실행 시 `Logger::shutdown()` 또는 Drop 전에 `flush()`를 호출하여 로그 유실을 방지하세요.

## Features

- **High-Performance Async Logging**: Non-blocking log operations with batched queue processing
- **Multiple Appenders**: Console, file, and custom log destinations
- **Thread-Safe Operations**: Concurrent logging from multiple threads
- **Zero-Copy Design**: Efficient message passing with minimal allocations
- **Flexible Log Levels**: Trace, Debug, Info, Warn, Error, Fatal
- **Beautiful Console Output**: ANSI colored output for better readability
- **Cross-Platform**: Works on Windows, Linux, and macOS
- **Builder Pattern**: Fluent API for constructing loggers (v0.1.1+)
- **Logging Macros**: Ergonomic macros for formatted logging (v0.1.1+)
- **Inline Optimizations**: `#[inline]` hints on hot paths for better performance (v0.1.1+)
- **Property Testing**: Robust validation using `proptest` (v0.1.1+)
- **Benchmarking**: Performance tracking with `criterion` (v0.1.1+)
- **Overflow Policies**: Configurable behavior when async queue is full (v0.2.0+)
- **Priority-Based Preservation**: Critical logs (Error, Fatal) are never dropped (v0.2.0+)
- **Logger Metrics**: Track dropped logs, queue full events, and drop rates (v0.2.0+)

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
rust_logger_system = "0.1"
```

### Basic Usage

```rust
use rust_logger_system::prelude::*;

fn main() -> Result<()> {
    // Create logger
    let mut logger = Logger::new();

    // Add console appender
    logger.add_appender(Box::new(ConsoleAppender::new()));

    // Log messages
    logger.info("Application started");
    logger.warn("This is a warning");
    logger.error("An error occurred");

    Ok(())
}
```

### Async Logging

```rust
use rust_logger_system::prelude::*;

fn main() -> Result<()> {
    // Create async logger with buffer size
    let mut logger = Logger::with_async(1000);

    logger.add_appender(Box::new(ConsoleAppender::new()));
    logger.add_appender(Box::new(FileAppender::new("app.log")?));

    logger.info("Async logging is fast!");

    Ok(())
}
```

### Builder Pattern (v0.1.1+)

Use the fluent builder API for ergonomic logger construction:

```rust
use rust_logger_system::prelude::*;

let logger = Logger::builder()
    .min_level(LogLevel::Debug)
    .appender(ConsoleAppender::new())
    .appender(FileAppender::new("app.log")?)
    .async_mode(1000)
    .build();

logger.info("Logger configured with builder pattern");
```

### Logging Macros (v0.1.1+)

Use convenient macros for formatted logging:

```rust
use rust_logger_system::{Logger, info, warn, error};

let logger = Logger::new();

// Basic logging
info!(logger, "Application started");

// With format arguments
let port = 8080;
info!(logger, "Server listening on port {}", port);

// Complex formatting
let user_id = 42;
let action = "login";
warn!(logger, "User {} performed action: {}", user_id, action);

// Error logging
let error_code = 500;
error!(logger, "Internal server error: code {}", error_code);
```

Available macros:
- `trace!(logger, ...)` - Trace-level logging
- `debug!(logger, ...)` - Debug-level logging
- `info!(logger, ...)` - Info-level logging
- `warn!(logger, ...)` - Warning-level logging
- `error!(logger, ...)` - Error-level logging
- `fatal!(logger, ...)` - Fatal-level logging

### Overflow Policies (v0.2.0+)

Configure how the logger handles a full async queue:

```rust
use rust_logger_system::prelude::*;
use std::sync::Arc;
use std::time::Duration;

// AlertAndDrop (default): Drop logs but alert operators
let logger = Logger::builder()
    .async_mode(1000)
    .overflow_policy(OverflowPolicy::AlertAndDrop)
    .on_overflow(Arc::new(|count| {
        eprintln!("ALERT: {} logs dropped!", count);
    }))
    .build();

// Block: Wait for queue space (use with caution)
let logger = Logger::builder()
    .async_mode(1000)
    .overflow_policy(OverflowPolicy::Block)
    .build();

// BlockWithTimeout: Wait up to N ms, then drop
let logger = Logger::builder()
    .async_mode(1000)
    .overflow_policy(OverflowPolicy::BlockWithTimeout(Duration::from_millis(100)))
    .build();

// DropNewest: Silently drop new logs (tracks metrics)
let logger = Logger::builder()
    .async_mode(1000)
    .overflow_policy(OverflowPolicy::DropNewest)
    .build();

// Check metrics
let metrics = logger.metrics();
println!("Dropped: {}", metrics.dropped_count());
println!("Total logged: {}", metrics.total_logged());
println!("Drop rate: {:.2}%", metrics.drop_rate());
```

**Note**: Critical logs (Error, Fatal) are **never dropped** regardless of overflow policy - they are force-written synchronously if the queue is full.

## Performance

### Async Logging Performance

- **Async mode**: ~50ns per log call (non-blocking)
- **Sync mode**: ~500ns per log call (blocking I/O)
- **Throughput**: 10M+ log messages/second in async mode
- **Memory overhead**: Configurable buffer size (default 1000 messages)
- **Batching**: Automatic log batching reduces I/O operations

### Best Practices

```rust
// ✅ DO: Use async logging in production for best performance
let logger = Logger::with_async(1000);  // Buffer size 1000

// ✅ DO: Use appropriate log levels
logger.debug("Detailed diagnostic information");  // Development only
logger.info("Normal operation");                  // Production
logger.error("Error that needs attention");       // Always logged

// ❌ DON'T: Use sync logging in hot paths
let logger = Logger::new();  // Blocks on every log call
```

## Security

### Log Injection Prevention

**⚠️ IMPORTANT**: Always sanitize user input before logging to prevent log injection attacks.

**✅ DO** sanitize user input:

```rust
use rust_logger_system::prelude::*;

let user_input = user_provided_data();  // Could contain newlines/control chars

// Safe: Sanitize before logging
let sanitized = user_input.replace('\n', " ").replace('\r', " ");
logger.info(&format!("User action: {}", sanitized));

// Or use structured logging (future enhancement)
```

**❌ DON'T** log unsanitized user input:

```rust
// UNSAFE: User could inject fake log entries
let user_input = "legitimate\nERROR: Fake admin login";
logger.info(&format!("Input: {}", user_input));  // DON'T DO THIS!
// Could produce misleading logs
```

### File System Security

- **File Permissions**: Log files are created with restrictive permissions (0600 on Unix)
- **Directory Traversal**: File paths are validated to prevent directory traversal attacks
- **Disk Space**: Monitor disk usage to prevent DoS via log flooding
- **Sensitive Data**: Never log passwords, API keys, or personal information

### Best Practices

1. **Sanitize user input**: Remove or escape newlines and control characters before logging
2. **Use log levels appropriately**: Don't log sensitive data even at debug level
3. **Rotate logs**: Implement log rotation to prevent unbounded disk usage
4. **Restrict access**: Ensure log files have appropriate permissions
5. **Rate limiting**: Consider rate limiting in user-facing applications
6. **Audit logging**: Use separate, append-only logs for security-critical events

```rust
use rust_logger_system::prelude::*;

let mut logger = Logger::with_async(1000);

// ✅ DO: Sanitize and validate before logging
fn safe_log_user_action(logger: &Logger, username: &str, action: &str) {
    // Validate username doesn't contain control characters
    if username.chars().any(|c| c.is_control()) {
        logger.warn("Attempted log injection detected");
        return;
    }
    logger.info(&format!("User '{}' performed: {}", username, action));
}

// ❌ DON'T: Log sensitive information
logger.info(&format!("Password: {}", password));  // NEVER DO THIS!
```

## License

BSD 3-Clause License - see LICENSE file for details.

---

Made with ❤️ in Rust
