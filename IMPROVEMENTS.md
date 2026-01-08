# Rust Logger System - Improvement Plan

> **Languages**: English | [한국어](./IMPROVEMENTS.ko.md)

## Overview

This document outlines identified weaknesses and proposed improvements for the Rust Logger System based on code analysis.

## Identified Issues

### 1. Log Loss on Queue Overflow

**Issue**: Async logging can silently drop logs if the internal channel fills up, leading to potential loss of critical diagnostic information during system overload or incident investigation.

**Location**: `src/logger.rs:234` (async mode)

**Current Implementation**:
```rust
pub fn log(&self, record: LogRecord) {
    if self.config.async_mode {
        // try_send fails silently if queue is full!
        let _ = self.sender.try_send(record);
    } else {
        self.write_log(record);
    }
}
```

**Impact**:
- Critical error messages may be lost during high load
- No notification when logs are dropped
- Difficult to debug issues when diagnostic data is missing
- Cannot trust log completeness during incident response

**Proposed Solution**:

**Option 1: Add overflow policy configuration**

```rust
// TODO: Add configurable overflow policy to prevent silent log loss

#[derive(Debug, Clone)]
pub enum OverflowPolicy {
    Drop,              // Drop new logs (current behavior)
    Block,             // Block until space available
    DropOldest,        // Drop oldest logs to make room
    AlertAndDrop,      // Drop but alert operators
}

#[derive(Debug, Clone)]
pub struct LoggerConfig {
    pub async_mode: bool,
    pub queue_size: usize,
    pub overflow_policy: OverflowPolicy,
    pub on_overflow: Option<Box<dyn Fn(usize) + Send + Sync>>,  // Callback
    // ... other config
}

impl Logger {
    pub fn log(&self, record: LogRecord) {
        if self.config.async_mode {
            match self.config.overflow_policy {
                OverflowPolicy::Drop => {
                    if self.sender.try_send(record).is_err() {
                        self.metrics.dropped_logs.fetch_add(1, Ordering::Relaxed);
                    }
                }
                OverflowPolicy::Block => {
                    // Block until space available
                    self.sender.send(record).ok();
                }
                OverflowPolicy::DropOldest => {
                    // Try non-blocking send first
                    if self.sender.try_send(record.clone()).is_err() {
                        // Queue full, drain one and retry
                        self.receiver.try_recv().ok();
                        self.metrics.dropped_logs.fetch_add(1, Ordering::Relaxed);
                        self.sender.try_send(record).ok();
                    }
                }
                OverflowPolicy::AlertAndDrop => {
                    if self.sender.try_send(record).is_err() {
                        let dropped = self.metrics.dropped_logs.fetch_add(1, Ordering::Relaxed);

                        // Alert on first drop and every 1000th drop
                        if dropped == 0 || dropped % 1000 == 0 {
                            if let Some(ref callback) = self.config.on_overflow {
                                callback(dropped);
                            }
                            eprintln!("WARNING: Logger queue full, {} logs dropped", dropped);
                        }
                    }
                }
            }
        } else {
            self.write_log(record);
        }
    }

    pub fn dropped_log_count(&self) -> usize {
        self.metrics.dropped_logs.load(Ordering::Relaxed)
    }
}
```

**Option 2: Add priority levels to preserve critical logs**

```rust
// TODO: Add priority system to preserve critical logs during overflow

pub enum LogPriority {
    Critical,  // Never drop
    High,      // Drop only as last resort
    Normal,    // Can drop under pressure
}

impl LogRecord {
    pub fn priority(&self) -> LogPriority {
        match self.level {
            LogLevel::Error | LogLevel::Fatal => LogPriority::Critical,
            LogLevel::Warn => LogPriority::High,
            _ => LogPriority::Normal,
        }
    }
}

impl Logger {
    pub fn log(&self, record: LogRecord) {
        if self.config.async_mode {
            let priority = record.priority();

            match self.sender.try_send(record) {
                Ok(()) => {}
                Err(TrySendError::Full(record)) => {
                    match priority {
                        LogPriority::Critical => {
                            // Force send by blocking - never drop critical logs
                            self.sender.send(record).ok();
                        }
                        LogPriority::High => {
                            // Try to drop a lower priority log first
                            if self.drop_lowest_priority_log() {
                                self.sender.try_send(record).ok();
                            }
                        }
                        LogPriority::Normal => {
                            // Drop this log
                            self.metrics.dropped_logs.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Err(TrySendError::Disconnected(_)) => {
                    // Logger shut down
                }
            }
        } else {
            self.write_log(record);
        }
    }
}
```

**Priority**: High
**Estimated Effort**: Medium (1 week)

### 2. Inconsistent Timestamp Formatting ✅ RESOLVED

**Issue**: Custom timestamp formats in log output are not validated or standardized, leading to parsing difficulties, inconsistent log analysis, and integration problems with log aggregation systems.

**Status**: **RESOLVED** - Implemented in `src/core/timestamp.rs`

**Solution Implemented**:
- Added `TimestampFormat` enum with support for ISO 8601, RFC 3339, Unix timestamps, and custom formats
- Added `FormatterConfig` struct for sharing formatting configuration
- Updated all appenders (ConsoleAppender, FileAppender, RotatingFileAppender, JsonAppender) to support configurable timestamp formats
- JSON output now supports both string and numeric timestamp formats based on configuration

**Previous Impact** (now resolved):
- Cannot customize timestamp format for different environments
- Missing timezone information in some formats
- Difficult to parse logs with automated tools
- Incompatible with common log aggregation systems (Elasticsearch, Splunk, etc.)

**Proposed Solution**:

```rust
// TODO: Add standardized, configurable timestamp formats

#[derive(Debug, Clone)]
pub enum TimestampFormat {
    Iso8601,           // 2025-10-17T10:30:45.123Z
    Iso8601WithMicros, // 2025-10-17T10:30:45.123456Z
    Rfc3339,           // 2025-10-17T10:30:45+00:00
    Unix,              // 1697536245
    UnixMillis,        // 1697536245123
    UnixMicros,        // 1697536245123456
    Custom(String),    // Custom strftime format
}

impl TimestampFormat {
    pub fn format(&self, timestamp: &SystemTime) -> String {
        match self {
            TimestampFormat::Iso8601 => {
                let datetime: DateTime<Utc> = (*timestamp).into();
                datetime.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
            }
            TimestampFormat::Iso8601WithMicros => {
                let datetime: DateTime<Utc> = (*timestamp).into();
                datetime.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
            }
            TimestampFormat::Rfc3339 => {
                let datetime: DateTime<Utc> = (*timestamp).into();
                datetime.to_rfc3339()
            }
            TimestampFormat::Unix => {
                timestamp
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .to_string()
            }
            TimestampFormat::UnixMillis => {
                timestamp
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    .to_string()
            }
            TimestampFormat::UnixMicros => {
                timestamp
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_micros()
                    .to_string()
            }
            TimestampFormat::Custom(format) => {
                let datetime: DateTime<Utc> = (*timestamp).into();
                datetime.format(format).to_string()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormatterConfig {
    pub timestamp_format: TimestampFormat,
    pub include_thread_id: bool,
    pub include_file_location: bool,
    // ... other options
}

impl Formatter {
    pub fn with_config(config: FormatterConfig) -> Self {
        Self { config }
    }

    pub fn format(&self, record: &LogRecord) -> String {
        let timestamp = self.config.timestamp_format.format(&record.timestamp);

        format!(
            "[{}] [{}] {}",
            timestamp,
            record.level,
            record.message
        )
    }
}
```

**Integration with structured logging**:

```rust
// TODO: Add structured logging support with JSON output

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Text,      // Human-readable text format
    Json,      // JSON for log aggregation
    Logfmt,    // Logfmt key=value format
}

impl Formatter {
    pub fn format_json(&self, record: &LogRecord) -> String {
        serde_json::json!({
            "timestamp": self.config.timestamp_format.format(&record.timestamp),
            "level": record.level.to_string(),
            "message": record.message,
            "thread": record.thread_id,
            "file": record.file,
            "line": record.line,
            "fields": record.fields,  // Additional structured fields
        }).to_string()
    }
}
```

**Priority**: Medium
**Estimated Effort**: Small (2-3 days)

### 3. No Log Rotation Strategy

**Issue**: File appender lacks built-in log rotation, which can lead to unbounded disk usage and make log management difficult in long-running applications.

**Current State**:
```rust
// src/appenders/file.rs
pub struct FileAppender {
    file: File,
    path: PathBuf,
    // No rotation support!
}
```

**Impact**:
- Log files can grow indefinitely
- Disk space exhaustion in production
- Difficult to manage and archive old logs
- Performance degradation with extremely large log files

**Proposed Solution**:

```rust
// TODO: Add log rotation strategies

#[derive(Debug, Clone)]
pub enum RotationPolicy {
    Size { max_bytes: u64 },                    // Rotate after N bytes
    Time { interval: Duration },                 // Rotate every N hours/days
    Daily { hour: u8 },                          // Rotate at specific time
    Hourly,                                      // Rotate every hour
    Hybrid { max_bytes: u64, interval: Duration }, // Rotate on size OR time
}

#[derive(Debug, Clone)]
pub struct FileAppenderConfig {
    pub path: PathBuf,
    pub rotation: Option<RotationPolicy>,
    pub max_backups: usize,                      // Keep N old files
    pub compress_backups: bool,                  // Gzip old files
}

pub struct FileAppender {
    config: FileAppenderConfig,
    current_file: File,
    current_size: u64,
    last_rotation: SystemTime,
}

impl FileAppender {
    pub fn write(&mut self, message: &str) -> std::io::Result<()> {
        // Check if rotation needed
        if self.should_rotate()? {
            self.rotate()?;
        }

        self.current_file.write_all(message.as_bytes())?;
        self.current_size += message.len() as u64;

        Ok(())
    }

    fn should_rotate(&self) -> std::io::Result<bool> {
        match &self.config.rotation {
            None => Ok(false),
            Some(RotationPolicy::Size { max_bytes }) => {
                Ok(self.current_size >= *max_bytes)
            }
            Some(RotationPolicy::Time { interval }) => {
                let elapsed = SystemTime::now()
                    .duration_since(self.last_rotation)
                    .unwrap();
                Ok(elapsed >= *interval)
            }
            Some(RotationPolicy::Daily { hour }) => {
                let now: DateTime<Local> = SystemTime::now().into();
                let last: DateTime<Local> = self.last_rotation.into();

                Ok(now.date() != last.date() && now.hour() >= *hour)
            }
            Some(RotationPolicy::Hourly) => {
                let elapsed = SystemTime::now()
                    .duration_since(self.last_rotation)
                    .unwrap();
                Ok(elapsed >= Duration::from_secs(3600))
            }
            Some(RotationPolicy::Hybrid { max_bytes, interval }) => {
                let size_exceeded = self.current_size >= *max_bytes;
                let time_exceeded = SystemTime::now()
                    .duration_since(self.last_rotation)
                    .unwrap() >= *interval;
                Ok(size_exceeded || time_exceeded)
            }
        }
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        // Close current file
        self.current_file.sync_all()?;

        // Rename current file to backup
        let backup_path = self.generate_backup_path();
        std::fs::rename(&self.config.path, &backup_path)?;

        // Compress if configured
        if self.config.compress_backups {
            self.compress_file(&backup_path)?;
        }

        // Clean up old backups
        self.cleanup_old_backups()?;

        // Open new file
        self.current_file = File::create(&self.config.path)?;
        self.current_size = 0;
        self.last_rotation = SystemTime::now();

        Ok(())
    }

    fn generate_backup_path(&self) -> PathBuf {
        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        let stem = self.config.path.file_stem().unwrap();
        let extension = self.config.path.extension().unwrap_or_default();

        self.config.path.with_file_name(
            format!("{}-{}.{}", stem.to_string_lossy(), timestamp, extension.to_string_lossy())
        )
    }

    fn cleanup_old_backups(&self) -> std::io::Result<()> {
        let parent = self.config.path.parent().unwrap();
        let stem = self.config.path.file_stem().unwrap().to_string_lossy();

        let mut backups: Vec<_> = std::fs::read_dir(parent)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.file_name()
                    .to_string_lossy()
                    .starts_with(stem.as_ref())
            })
            .collect();

        // Sort by modification time
        backups.sort_by_key(|entry| {
            entry.metadata().unwrap().modified().unwrap()
        });

        // Remove oldest files beyond max_backups
        if backups.len() > self.config.max_backups {
            for entry in &backups[..backups.len() - self.config.max_backups] {
                std::fs::remove_file(entry.path())?;
            }
        }

        Ok(())
    }
}
```

**Usage Example**:
```rust
let appender = FileAppender::with_config(FileAppenderConfig {
    path: PathBuf::from("/var/log/myapp.log"),
    rotation: Some(RotationPolicy::Hybrid {
        max_bytes: 100 * 1024 * 1024,  // 100 MB
        interval: Duration::from_secs(24 * 3600),  // 24 hours
    }),
    max_backups: 7,  // Keep one week of logs
    compress_backups: true,
});
```

**Priority**: Medium
**Estimated Effort**: Medium (1 week)

## Additional Improvements

### 4. Context and Structured Fields

**Suggestion**: Add support for structured logging with contextual fields:

```rust
// TODO: Add structured logging support

#[derive(Debug, Clone)]
pub struct LogRecord {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: SystemTime,
    pub fields: HashMap<String, serde_json::Value>,  // Structured fields
    // ... other fields
}

impl Logger {
    pub fn with_fields(&self, fields: HashMap<String, serde_json::Value>) -> LoggerContext {
        LoggerContext {
            logger: self,
            fields,
        }
    }
}

pub struct LoggerContext<'a> {
    logger: &'a Logger,
    fields: HashMap<String, serde_json::Value>,
}

impl<'a> LoggerContext<'a> {
    pub fn info(&self, message: &str) {
        let mut record = LogRecord::new(LogLevel::Info, message);
        record.fields = self.fields.clone();
        self.logger.log(record);
    }

    pub fn with_field(mut self, key: &str, value: serde_json::Value) -> Self {
        self.fields.insert(key.to_string(), value);
        self
    }
}

// Usage:
logger
    .with_fields(hashmap! {
        "request_id" => json!("abc-123"),
        "user_id" => json!(42),
    })
    .info("User logged in");

// Output (JSON format):
// {"timestamp":"2025-10-17T10:30:45Z","level":"INFO","message":"User logged in","request_id":"abc-123","user_id":42}
```

**Priority**: Low
**Estimated Effort**: Medium (1 week)

### 5. Sampling for High-Volume Logs

**Suggestion**: Add log sampling to reduce volume while maintaining visibility:

```rust
// TODO: Add log sampling for high-volume scenarios

#[derive(Debug, Clone)]
pub struct SamplingConfig {
    pub rate: f64,           // Sample rate 0.0 to 1.0
    pub always_sample: Vec<LogLevel>,  // Always log these levels
}

impl Logger {
    pub fn should_sample(&self, record: &LogRecord) -> bool {
        // Always sample configured levels
        if self.config.sampling.always_sample.contains(&record.level) {
            return true;
        }

        // Probabilistic sampling
        rand::random::<f64>() < self.config.sampling.rate
    }

    pub fn log(&self, record: LogRecord) {
        if !self.should_sample(&record) {
            self.metrics.sampled_logs.fetch_add(1, Ordering::Relaxed);
            return;
        }

        // ... actual logging
    }
}

// Usage:
let logger = Logger::with_config(LoggerConfig {
    sampling: SamplingConfig {
        rate: 0.1,  // Sample 10% of logs
        always_sample: vec![LogLevel::Error, LogLevel::Fatal],  // Always log errors
    },
    // ... other config
});
```

**Priority**: Low
**Estimated Effort**: Small (2-3 days)

## Testing Requirements

### New Tests Needed:

1. **Overflow Policy Tests**:
   ```rust
   #[test]
   fn test_drop_oldest_policy() {
       let logger = Logger::with_config(LoggerConfig {
           async_mode: true,
           queue_size: 10,
           overflow_policy: OverflowPolicy::DropOldest,
           ..Default::default()
       });

       // Fill queue
       for i in 0..15 {
           logger.info(&format!("Message {}", i));
       }

       // Wait for processing
       logger.flush();

       // Should have dropped oldest 5 messages
       assert_eq!(logger.dropped_log_count(), 5);
   }

   #[test]
   fn test_critical_logs_never_dropped() {
       let logger = Logger::with_config(LoggerConfig {
           async_mode: true,
           queue_size: 10,
           overflow_policy: OverflowPolicy::PriorityBased,
           ..Default::default()
       });

       // Fill queue with info logs
       for i in 0..10 {
           logger.info("Filler");
       }

       // Critical log should force through
       logger.error("Critical error");

       logger.flush();

       // Should have log content with critical error
       // and some info logs dropped
   }
   ```

2. **Timestamp Format Tests**:
   ```rust
   #[test]
   fn test_timestamp_formats() {
       let record = LogRecord::new(LogLevel::Info, "test");

       let formats = vec![
           TimestampFormat::Iso8601,
           TimestampFormat::Rfc3339,
           TimestampFormat::Unix,
       ];

       for format in formats {
           let timestamp = format.format(&record.timestamp);

           // Verify parseable
           match format {
               TimestampFormat::Iso8601 => {
                   DateTime::parse_from_rfc3339(&timestamp).unwrap();
               }
               TimestampFormat::Unix => {
                   timestamp.parse::<u64>().unwrap();
               }
               _ => {}
           }
       }
   }
   ```

3. **Log Rotation Tests**:
   ```rust
   #[test]
   fn test_size_based_rotation() {
       let temp_dir = tempdir().unwrap();
       let log_path = temp_dir.path().join("test.log");

       let mut appender = FileAppender::with_config(FileAppenderConfig {
           path: log_path.clone(),
           rotation: Some(RotationPolicy::Size {
               max_bytes: 1024,  // 1 KB
           }),
           max_backups: 3,
           compress_backups: false,
       });

       // Write 5 KB of logs
       for _ in 0..50 {
           appender.write(&"x".repeat(100)).unwrap();
       }

       // Should have rotated and created backup files
       let backups: Vec<_> = std::fs::read_dir(temp_dir.path())
           .unwrap()
           .filter_map(|e| e.ok())
           .filter(|e| e.file_name() != "test.log")
           .collect();

       assert!(backups.len() >= 3);
   }
   ```

## Implementation Roadmap

### Phase 1: Critical Reliability (Sprint 1) ✅ COMPLETED
- [x] Implement overflow policies (DropNewest, DropOldest, Block, BlockWithTimeout, AlertAndDrop)
- [x] Add dropped log metrics (LoggerMetrics with dropped_count, total_logged, queue_full_events)
- [x] Add overflow alerts (on_overflow callback and stderr warnings)
- [x] Test all overflow scenarios
- [x] Add priority-based preservation (Critical logs never dropped)
- [x] Add PriorityConfig for configurable priority behavior:
  - `preserve_critical`: Control whether Error/Fatal are force-written
  - `preserve_high`: Enable retry mechanism for Warn logs
  - `block_on_critical`: Control blocking behavior for critical logs
  - `high_priority_retry_count`: Configurable retry attempts for high priority
- [x] Add stress tests for priority preservation under load
- [x] Update documentation with PriorityConfig examples

### Phase 2: Formatting and Standards (Sprint 2) ✅ COMPLETED
- [x] Add standard timestamp formats (TimestampFormat enum with Iso8601, Rfc3339, Unix variants)
- [x] Implement JSON output format (JsonAppender with configurable timestamp)
- [x] Add structured logging support (LogContext with FieldValue)
- [x] Update documentation

### Phase 3: Production Features (Sprint 3)
- [ ] Implement log rotation
- [ ] Add compression support
- [ ] Create rotation tests
- [ ] Add operations guide

### Phase 4: Advanced Features (Sprint 4)
- [ ] Add log sampling
- [ ] Implement context propagation
- [ ] Add performance benchmarks
- [ ] Create advanced examples

## Breaking Changes

⚠️ **Note**: Changing default overflow behavior may affect existing deployments.

**Migration Path**:
1. Version 1.x: Add new overflow policies with current behavior as default
2. Version 1.x: Add deprecation warnings for silent drops
3. Version 2.0: Change default to AlertAndDrop policy
4. Document migration in CHANGELOG

## Performance Targets

### Current Performance:
- Sync logging: ~100k logs/sec
- Async logging: ~500k logs/sec
- Log loss: Unknown (not tracked)

### Target Performance After Improvements:
- Sync logging: ~100k logs/sec (unchanged)
- Async logging: ~500k logs/sec (unchanged)
- Log loss: 0% for critical logs, <0.1% for normal logs
- Dropped log tracking: 100% accurate
- Rotation overhead: <1ms per rotation

## References

- Code Analysis: Logger System Review 2025-10-16
- Related Issues:
  - Log loss (#TODO)
  - Timestamp formatting (#TODO)
  - Log rotation (#TODO)
- slog documentation: https://docs.rs/slog/
- tracing documentation: https://docs.rs/tracing/

---

*Improvement Plan Version 1.0*
*Last Updated: 2025-10-17*
