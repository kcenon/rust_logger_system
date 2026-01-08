//! Criterion benchmarks for rust_logger_system

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use rust_logger_system::prelude::*;
use std::sync::Arc;

// ============================================================================
// Logger Creation Benchmarks
// ============================================================================

fn bench_logger_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("logger_creation");
    group.throughput(Throughput::Elements(1));

    group.bench_function("new_sync", |b| {
        b.iter(|| {
            let logger = Logger::new();
            black_box(logger)
        });
    });

    group.bench_function("new_async", |b| {
        b.iter(|| {
            let logger = Logger::with_async(1000);
            black_box(logger)
        });
    });

    group.finish();
}

// ============================================================================
// Logging Performance Benchmarks
// ============================================================================

fn bench_sync_logging(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync_logging");
    group.throughput(Throughput::Elements(1));

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Trace);

    group.bench_function("trace", |b| {
        b.iter(|| {
            logger.trace(black_box("Trace message"));
        });
    });

    group.bench_function("debug", |b| {
        b.iter(|| {
            logger.debug(black_box("Debug message"));
        });
    });

    group.bench_function("info", |b| {
        b.iter(|| {
            logger.info(black_box("Info message"));
        });
    });

    group.bench_function("warn", |b| {
        b.iter(|| {
            logger.warn(black_box("Warning message"));
        });
    });

    group.bench_function("error", |b| {
        b.iter(|| {
            logger.error(black_box("Error message"));
        });
    });

    group.finish();
}

fn bench_async_logging(c: &mut Criterion) {
    let mut group = c.benchmark_group("async_logging");
    group.throughput(Throughput::Elements(1));

    let mut logger = Logger::with_async(10000);
    logger.set_min_level(LogLevel::Trace);

    group.bench_function("info", |b| {
        b.iter(|| {
            logger.info(black_box("Info message"));
        });
    });

    group.bench_function("error", |b| {
        b.iter(|| {
            logger.error(black_box("Error message"));
        });
    });

    group.finish();
}

// ============================================================================
// Concurrent Logging Benchmarks
// ============================================================================

fn bench_concurrent_logging(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_logging");

    let logger = Arc::new(Logger::with_async(10000));

    group.bench_function("single_thread", |b| {
        let logger = Arc::clone(&logger);
        b.iter(|| {
            logger.info(black_box("Concurrent message"));
        });
    });

    group.bench_function("multi_thread_4", |b| {
        let logger = Arc::clone(&logger);
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let logger = Arc::clone(&logger);
                    std::thread::spawn(move || {
                        logger.info(black_box("Concurrent message"));
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Log Entry Creation Benchmarks
// ============================================================================

fn bench_log_entry_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("log_entry_creation");
    group.throughput(Throughput::Elements(1));

    group.bench_function("new", |b| {
        b.iter(|| {
            let entry = LogEntry::new(
                black_box(LogLevel::Info),
                black_box("Test message".to_string()),
            );
            black_box(entry)
        });
    });

    group.bench_function("with_context", |b| {
        b.iter(|| {
            let entry = LogEntry::new(
                black_box(LogLevel::Info),
                black_box("Test message".to_string()),
            )
            .with_location(
                black_box("test.rs"),
                black_box(42),
                black_box("test_module"),
            );
            black_box(entry)
        });
    });

    group.finish();
}

// ============================================================================
// Serialization Benchmarks
// ============================================================================

fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");
    group.throughput(Throughput::Elements(1));

    let entry = LogEntry::new(LogLevel::Info, "Test message".to_string());

    group.bench_function("to_json", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&entry).unwrap();
            black_box(json)
        });
    });

    group.bench_function("to_json_pretty", |b| {
        b.iter(|| {
            let json = serde_json::to_string_pretty(&entry).unwrap();
            black_box(json)
        });
    });

    group.finish();
}

// ============================================================================
// Filtering Benchmarks
// ============================================================================

fn bench_level_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("level_filtering");
    group.throughput(Throughput::Elements(1));

    let mut logger = Logger::new();
    logger.set_min_level(LogLevel::Warn);

    group.bench_function("below_threshold", |b| {
        b.iter(|| {
            logger.debug(black_box("This should be filtered"));
        });
    });

    group.bench_function("above_threshold", |b| {
        b.iter(|| {
            logger.error(black_box("This should be logged"));
        });
    });

    group.finish();
}

// ============================================================================
// Sampling Benchmarks
// ============================================================================

fn bench_sampling(c: &mut Criterion) {
    let mut group = c.benchmark_group("sampling");
    group.throughput(Throughput::Elements(1));

    // No sampling (baseline)
    let mut logger_no_sampling = Logger::builder().build();
    logger_no_sampling.set_min_level(LogLevel::Info);

    group.bench_function("no_sampling", |b| {
        b.iter(|| {
            logger_no_sampling.info(black_box("Message without sampling"));
        });
    });

    // 50% sampling
    let mut logger_50pct = Logger::builder().sample_rate(0.5).build();
    logger_50pct.set_min_level(LogLevel::Info);

    group.bench_function("50pct_sampling", |b| {
        b.iter(|| {
            logger_50pct.info(black_box("Message with 50% sampling"));
        });
    });

    // 10% sampling
    let mut logger_10pct = Logger::builder().sample_rate(0.1).build();
    logger_10pct.set_min_level(LogLevel::Info);

    group.bench_function("10pct_sampling", |b| {
        b.iter(|| {
            logger_10pct.info(black_box("Message with 10% sampling"));
        });
    });

    // Always-sample level (Error should always be logged)
    let mut logger_critical = Logger::builder()
        .with_sampling(SamplingConfig::new(0.0)) // Drop all except critical
        .build();
    logger_critical.set_min_level(LogLevel::Info);

    group.bench_function("always_sample_critical", |b| {
        b.iter(|| {
            logger_critical.error(black_box("Error message - always sampled"));
        });
    });

    group.bench_function("dropped_by_sampling", |b| {
        b.iter(|| {
            logger_critical.info(black_box("Info message - dropped by sampling"));
        });
    });

    group.finish();
}

fn bench_sampling_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("sampling_overhead");
    group.throughput(Throughput::Elements(100));

    // Measure overhead of sampling vs no sampling for 100 messages
    group.bench_function("100_messages_no_sampling", |b| {
        let mut logger = Logger::builder().build();
        logger.set_min_level(LogLevel::Info);

        b.iter(|| {
            for i in 0..100 {
                logger.info(black_box(format!("Message {}", i)));
            }
        });
    });

    group.bench_function("100_messages_with_100pct_sampling", |b| {
        let mut logger = Logger::builder().sample_rate(1.0).build();
        logger.set_min_level(LogLevel::Info);

        b.iter(|| {
            for i in 0..100 {
                logger.info(black_box(format!("Message {}", i)));
            }
        });
    });

    group.bench_function("100_messages_with_50pct_sampling", |b| {
        let mut logger = Logger::builder().sample_rate(0.5).build();
        logger.set_min_level(LogLevel::Info);

        b.iter(|| {
            for i in 0..100 {
                logger.info(black_box(format!("Message {}", i)));
            }
        });
    });

    group.finish();
}

fn bench_sampler_direct(c: &mut Criterion) {
    use rust_logger_system::LogSampler;

    let mut group = c.benchmark_group("sampler_direct");
    group.throughput(Throughput::Elements(1));

    let sampler = LogSampler::new(SamplingConfig::new(0.5));

    group.bench_function("should_sample_info", |b| {
        b.iter(|| {
            let result = sampler.should_sample(black_box(LogLevel::Info), None);
            black_box(result)
        });
    });

    group.bench_function("should_sample_error", |b| {
        b.iter(|| {
            let result = sampler.should_sample(black_box(LogLevel::Error), None);
            black_box(result)
        });
    });

    group.bench_function("should_sample_with_category", |b| {
        b.iter(|| {
            let result = sampler.should_sample(black_box(LogLevel::Info), Some("database"));
            black_box(result)
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_logger_creation,
    bench_sync_logging,
    bench_async_logging,
    bench_concurrent_logging,
    bench_log_entry_creation,
    bench_serialization,
    bench_level_filtering,
    bench_sampling,
    bench_sampling_overhead,
    bench_sampler_direct
);

criterion_main!(benches);
