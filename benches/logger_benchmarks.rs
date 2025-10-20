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
    bench_level_filtering
);

criterion_main!(benches);
