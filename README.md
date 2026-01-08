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
- **Log Rotation**: Multiple rotation strategies - Size, Time, Daily, Hourly, Hybrid (v0.2.1+)
- **Structured Logging**: Type-safe fields with context propagation (v0.3.0+)
- **Output Formats**: Text, JSON, and Logfmt output formats (v0.3.0+)
- **Scoped Context**: RAII-based context management with automatic cleanup (v0.3.0+)
- **Log Sampling**: Configurable sampling for high-volume scenarios (v0.4.0+)

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

### Priority-Based Log Preservation (v0.2.0+)

Fine-tune how different log priorities are handled during queue overflow:

```rust
use rust_logger_system::prelude::*;

// Default configuration: preserve all critical and high priority logs
let logger = Logger::builder()
    .async_mode(100)
    .priority_config(PriorityConfig::default())
    .build();

// Custom configuration for high-throughput scenarios
let logger = Logger::builder()
    .async_mode(100)
    .priority_config(PriorityConfig {
        preserve_critical: true,   // Error/Fatal always written synchronously
        preserve_high: true,       // Warn logs get retry attempts
        block_on_critical: true,   // Block thread for critical logs if needed
        high_priority_retry_count: 5, // Retry Warn logs up to 5 times
    })
    .build();

// Minimal overhead configuration (only protect critical logs)
let logger = Logger::builder()
    .async_mode(100)
    .priority_config(PriorityConfig {
        preserve_critical: true,
        preserve_high: false,      // Don't retry Warn logs
        block_on_critical: false,  // Non-blocking critical writes
        high_priority_retry_count: 0,
    })
    .build();
```

**Priority Levels**:
- **Critical** (Error, Fatal): Never dropped when `preserve_critical=true`
- **High** (Warn): Retried before dropping when `preserve_high=true`
- **Normal** (Trace, Debug, Info): Subject to overflow policy

### Log Rotation (v0.2.1+)

Configure automatic log rotation with various strategies:

```rust
use rust_logger_system::appenders::{RotatingFileAppender, RotationPolicy, RotationStrategy};
use std::time::Duration;

// Size-based rotation (default behavior)
let policy = RotationPolicy::new()
    .with_max_size(100 * 1024 * 1024)  // 100 MB
    .with_max_backups(7)
    .with_compression(true);
let appender = RotatingFileAppender::with_policy("/var/log/app.log", policy)?;

// Time-based rotation (every hour)
let policy = RotationPolicy::new()
    .with_strategy(RotationStrategy::Time {
        interval: Duration::from_secs(3600)
    })
    .with_max_backups(24);

// Daily rotation at midnight
let policy = RotationPolicy::new()
    .with_strategy(RotationStrategy::Daily { hour: 0 })
    .with_max_backups(30)
    .with_compression(true);

// Hourly rotation
let policy = RotationPolicy::new()
    .with_strategy(RotationStrategy::Hourly)
    .with_max_backups(48);

// Hybrid: rotate on size OR time, whichever comes first
let policy = RotationPolicy::new()
    .with_strategy(RotationStrategy::Hybrid {
        max_bytes: 50 * 1024 * 1024,  // 50 MB
        interval: Duration::from_secs(24 * 3600),  // 24 hours
    })
    .with_max_backups(14)
    .with_compression(true);
```

**Available Rotation Strategies**:
- **Size**: Rotate when file exceeds specified bytes
- **Time**: Rotate at specified time intervals
- **Daily**: Rotate daily at specified hour (0-23)
- **Hourly**: Rotate every hour
- **Hybrid**: Rotate on size OR time, whichever comes first
- **Never**: Disable rotation (for external rotation management)

### Structured Logging (v0.3.0+)

Add type-safe fields to your log entries for better analysis and filtering:

```rust
use rust_logger_system::prelude::*;

// Set persistent context fields (added to all logs)
let logger = Logger::builder()
    .appender(ConsoleAppender::new())
    .build();

logger.context().set("service", "api-gateway");
logger.context().set("version", "1.2.3");

// Use the structured log builder
logger.info_builder()
    .message("Request processed")
    .field("user_id", 12345)
    .field("latency_ms", 42.5)
    .field("status", 200)
    .log();

// Or use LogContext directly
let ctx = LogContext::new()
    .with_field("request_id", "abc-123")
    .with_field("method", "POST");
logger.info_with_context("API call completed", ctx);
```

### Scoped Context (v0.3.0+)

Use RAII guards for automatic context cleanup:

```rust
use rust_logger_system::prelude::*;

let logger = Logger::new();

// Context automatically removed when guard is dropped
{
    let _guard = logger.with_scoped_context("request_id", "req-456");
    logger.info("Processing request");  // Includes request_id
    logger.info("Validating input");    // Includes request_id
}
// request_id automatically removed here

logger.info("Ready for next request");  // No request_id
```

### Output Formats (v0.3.0+)

Choose between Text, JSON, and Logfmt output formats:

```rust
use rust_logger_system::prelude::*;

// JSON format for log aggregation (ELK, Loki, etc.)
let logger = Logger::builder()
    .appender(ConsoleAppender::new().with_output_format(OutputFormat::Json))
    .build();

logger.info_builder()
    .message("User logged in")
    .field("user_id", 42)
    .log();
// Output: {"timestamp":"2025-01-08T10:30:45Z","level":"INFO","message":"User logged in","user_id":42}

// Logfmt format (key=value pairs)
let logger = Logger::builder()
    .appender(ConsoleAppender::new().with_output_format(OutputFormat::Logfmt))
    .build();

logger.info("Server started");
// Output: timestamp=2025-01-08T10:30:45Z level=INFO message="Server started"
```

**Available Output Formats**:
- **Text** (default): Human-readable format with optional colors
- **Json**: Machine-readable JSON, compatible with log aggregation tools
- **Logfmt**: Key=value format, simple and parseable

### Log Sampling (v0.4.0+)

Reduce log volume in high-throughput scenarios while ensuring critical logs are never dropped:

```rust
use rust_logger_system::prelude::*;
use std::collections::HashMap;

// Simple 50% sampling
let logger = Logger::builder()
    .sample_rate(0.5)  // Log 50% of messages
    .appender(ConsoleAppender::new())
    .build();

// Full configuration with category-specific rates
let logger = Logger::builder()
    .with_sampling(SamplingConfig {
        rate: 0.1,  // Default: sample 10% of logs
        always_sample: vec![LogLevel::Warn, LogLevel::Error, LogLevel::Fatal],
        category_rates: {
            let mut m = HashMap::new();
            m.insert("database".to_string(), 0.01);   // 1% of DB logs
            m.insert("security".to_string(), 1.0);    // 100% of security logs
            m
        },
        adaptive: true,              // Enable adaptive sampling
        adaptive_threshold: 50000,   // Threshold: 50k msgs/sec
        adaptive_min_rate: 0.001,    // Never go below 0.1%
    })
    .build();

// Check sampling metrics
if let Some(sampler) = logger.sampler() {
    let metrics = sampler.metrics();
    println!("Sampled: {}", metrics.sampled_count());
    println!("Dropped: {}", metrics.dropped_count());
    println!("Effective rate: {:.2}%", sampler.effective_sample_rate() * 100.0);
}
```

**Sampling Features**:
- **Random Sampling**: Configurable rate (0.0 to 1.0)
- **Always-Sample Levels**: Critical logs (Error, Fatal) never dropped by default
- **Category-Based Rates**: Different rates for different log categories
- **Adaptive Sampling**: Automatically reduces rate under high load
- **Metrics**: Track sampled/dropped counts for observability

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
