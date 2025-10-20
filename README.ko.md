# Rust Logger System

[English](README.md) | [한국어](README.ko.md)

비동기 처리, 다중 출력 대상, 최소한의 오버헤드로 포괄적인 로깅 기능을 제공하도록 설계된 프로덕션 수준의 고성능 Rust 로깅 프레임워크입니다.

이것은 [logger_system](https://github.com/kcenon/logger_system) 프로젝트의 Rust 구현으로, Rust의 안전성 보장과 성능 이점을 활용하여 동일한 기능을 제공합니다.

## 주요 기능

- **고성능 비동기 로깅**: 배치 큐 처리를 통한 논블로킹 로그 작업
- **다중 Appender**: Console, file, 커스텀 로그 대상
- **스레드 안전 작업**: 여러 스레드에서의 동시 로깅
- **Zero-Copy 설계**: 최소한의 할당으로 효율적인 메시지 전달
- **유연한 로그 레벨**: Trace, Debug, Info, Warn, Error, Fatal
- **아름다운 Console 출력**: 가독성 향상을 위한 ANSI 컬러 출력
- **크로스 플랫폼**: Windows, Linux, macOS에서 동작

## 빠른 시작

`Cargo.toml`에 다음을 추가하세요:

```toml
[dependencies]
rust_logger_system = "0.1"
```

### 기본 사용법

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

### 비동기 로깅

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

## 라이선스

BSD 3-Clause License - 자세한 내용은 LICENSE 파일을 참조하세요.

---

Made with ❤️ in Rust
