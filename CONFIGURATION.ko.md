# Rust Logger System 설정 가이드

> **Languages**: [English](./CONFIGURATION.md) | 한국어

## 개요

이 가이드는 Rust Logger System의 모든 설정 옵션을 다룹니다. 로거 모드, 로그 레벨, appender, 고급 설정 시나리오를 포함합니다.

## 목차

1. [로거 모드](#로거-모드)
2. [로그 레벨](#로그-레벨)
3. [Appender](#appender)
4. [설정 패턴](#설정-패턴)
5. [환경별 설정](#환경별-설정)
6. [성능 튜닝](#성능-튜닝)
7. [문제 해결](#문제-해결)

## 로거 모드

### 동기 로거

**사용 시기:**
- 간단한 애플리케이션
- 적은 로그 볼륨
- 즉각적인 로그 출력 필요
- 디버깅

**예제:**
```rust
use rust_logger_system::prelude::*;

let mut logger = Logger::new();
logger.add_appender(Box::new(ConsoleAppender::new()));
logger.set_min_level(LogLevel::Info);
```

**특징:**
- 로그 즉시 기록
- I/O 중 호출 스레드 블로킹
- 간단하고 예측 가능
- 낮은 처리량

### 비동기 로거

**사용 시기:**
- 고성능 애플리케이션
- 높은 로그 볼륨
- I/O 블로킹을 피해야 할 때
- 프로덕션 환경

**예제:**
```rust
use rust_logger_system::prelude::*;

let mut logger = Logger::with_async(10000); // 버퍼 크기: 10000
logger.add_appender(Box::new(FileAppender::new("app.log")?));
logger.set_min_level(LogLevel::Info);
```

**특징:**
- 논블로킹 로그 작업
- 백그라운드 워커 스레드
- 제한된 채널 버퍼
- 높은 처리량

**버퍼 크기 가이드라인:**
- 소형 앱 (< 100 logs/sec): 1,000
- 중형 앱 (100-1000 logs/sec): 10,000
- 대형 앱 (> 1000 logs/sec): 100,000

## 로그 레벨

### 레벨 계층 구조

```
Trace < Debug < Info < Warn < Error < Fatal
```

### 레벨 정의

| 레벨 | 값 | 사용 사례 | 예시 |
|-------|-------|----------|----------|
| **Trace** | 0 | 매우 상세한 디버깅 | 함수 진입/종료, 변수 값 |
| **Debug** | 1 | 개발 디버깅 | 상태 변화, 내부 작업 |
| **Info** | 2 | 일반 정보 | 서비스 시작, 설정 로드 |
| **Warn** | 3 | 경고 조건 | deprecated API 사용, 기본 설정 |
| **Error** | 4 | 에러 조건 | 실패한 작업, 예외 |
| **Fatal** | 5 | 치명적 실패 | 시스템 크래시, 데이터 손상 |

### 최소 레벨 설정

```rust
// Info 이상만 로깅됩니다
logger.set_min_level(LogLevel::Info);

logger.trace("로깅 안됨");  // 무시됨
logger.debug("로깅 안됨");  // 무시됨
logger.info("로깅됨");       // 기록됨
logger.warn("로깅됨");       // 기록됨
logger.error("로깅됨");      // 기록됨
```

### 동적 레벨 변경

```rust
// Info 레벨로 시작
logger.set_min_level(LogLevel::Info);

// 디버그 로깅 동적 활성화
logger.set_min_level(LogLevel::Debug);

// 모든 로깅 비활성화
logger.set_min_level(LogLevel::Fatal);
```

### 환경별 레벨 설정

```rust
use std::env;

let log_level = match env::var("LOG_LEVEL").as_deref() {
    Ok("trace") => LogLevel::Trace,
    Ok("debug") => LogLevel::Debug,
    Ok("warn") => LogLevel::Warn,
    Ok("error") => LogLevel::Error,
    _ => LogLevel::Info, // 기본값
};

logger.set_min_level(log_level);
```

## Appender

### Console Appender

ANSI 컬러 지원과 함께 stdout으로 로그 출력.

```rust
use rust_logger_system::prelude::*;

let console = ConsoleAppender::new();
logger.add_appender(Box::new(console));
```

**설정:**
- 자동 ANSI 컬러 지원
- UTF-8 인코딩
- 버퍼 없는 출력 (즉시 가시성)

**컬러 스킴:**
- Trace: 회색
- Debug: 청록색
- Info: 녹색
- Warn: 노란색
- Error: 빨간색
- Fatal: 밝은 빨간색 (굵게)

### File Appender

파일에 로그 작성.

```rust
use rust_logger_system::prelude::*;

let file = FileAppender::new("application.log")?;
logger.add_appender(Box::new(file));
```

**설정 옵션:**

#### 기본 파일 로깅
```rust
let file = FileAppender::new("app.log")?;
```

#### 커스텀 경로
```rust
use std::path::PathBuf;

let log_dir = PathBuf::from("/var/log/myapp");
std::fs::create_dir_all(&log_dir)?;
let file = FileAppender::new(log_dir.join("app.log"))?;
```

#### 다중 파일
```rust
// 일반 로그
logger.add_appender(Box::new(FileAppender::new("app.log")?));

// 에러 전용 로그 (애플리케이션 코드에서 필터링)
logger.add_appender(Box::new(FileAppender::new("errors.log")?));
```

**파일 포맷:**
```
[2025-10-16 10:30:45.123] [INFO] 애플리케이션 시작됨
[2025-10-16 10:30:45.456] [DEBUG] 설정 로드됨: config.toml
[2025-10-16 10:30:46.789] [ERROR] 연결 실패: Connection refused
```

### 다중 Appender

```rust
// 콘솔과 파일 모두에 로그
logger.add_appender(Box::new(ConsoleAppender::new()));
logger.add_appender(Box::new(FileAppender::new("app.log")?));

// 모든 로그가 두 곳에 모두 전달됩니다
logger.info("콘솔과 파일에 모두 나타남");
```

### 커스텀 Appender

`Appender` trait 구현:

```rust
use rust_logger_system::prelude::*;

struct NetworkAppender {
    endpoint: String,
}

impl Appender for NetworkAppender {
    fn append(&mut self, entry: &LogEntry) -> Result<()> {
        // 원격 서버로 로그 전송
        let json = serde_json::json!({
            "timestamp": entry.timestamp,
            "level": format!("{:?}", entry.level),
            "message": entry.message,
        });

        // HTTP 등으로 전송
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        // 모든 로그 전송 완료 보장
        Ok(())
    }
}

// 커스텀 appender 사용
logger.add_appender(Box::new(NetworkAppender {
    endpoint: "https://logs.example.com/ingest".to_string(),
}));
```

## 설정 패턴

### 개발 환경 설정

```rust
use rust_logger_system::prelude::*;

fn create_dev_logger() -> Logger {
    let mut logger = Logger::new(); // 디버깅용 동기식

    logger.add_appender(Box::new(ConsoleAppender::new()));
    logger.set_min_level(LogLevel::Debug); // 상세한 로깅

    logger
}
```

### 프로덕션 환경 설정

```rust
use rust_logger_system::prelude::*;

fn create_prod_logger() -> Result<Logger> {
    let mut logger = Logger::with_async(10000); // 고성능 비동기

    // 컨테이너 로그용 콘솔
    logger.add_appender(Box::new(ConsoleAppender::new()));

    // 영구 로그용 파일
    logger.add_appender(Box::new(FileAppender::new("/var/log/app/app.log")?));

    logger.set_min_level(LogLevel::Info); // 프로덕션 레벨

    Ok(logger)
}
```

### 테스트 환경 설정

```rust
use rust_logger_system::prelude::*;

fn create_test_logger() -> Logger {
    let mut logger = Logger::new();

    // 테스트에서는 콘솔 출력 없음
    logger.set_min_level(LogLevel::Warn); // 경고와 에러만

    logger
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_logging() {
        let logger = create_test_logger();

        // 최소한의 로깅으로 테스트 로직
    }
}
```

## 환경별 설정

### 환경 변수 사용

```rust
use std::env;
use rust_logger_system::prelude::*;

fn create_logger_from_env() -> Result<Logger> {
    // 환경에서 설정 읽기
    let log_level = env::var("LOG_LEVEL")
        .unwrap_or_else(|_| "info".to_string());

    let log_file = env::var("LOG_FILE")
        .unwrap_or_else(|_| "app.log".to_string());

    let async_mode = env::var("LOG_ASYNC")
        .map(|v| v == "true")
        .unwrap_or(true);

    // 환경 기반 로거 생성
    let mut logger = if async_mode {
        Logger::with_async(10000)
    } else {
        Logger::new()
    };

    // 레벨 설정
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

    // Appender 추가
    logger.add_appender(Box::new(ConsoleAppender::new()));

    if !log_file.is_empty() {
        logger.add_appender(Box::new(FileAppender::new(&log_file)?));
    }

    Ok(logger)
}
```

### 설정 파일 예제

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

    // 설정에서 레벨 설정
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

    // 설정에서 appender 추가
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

## 성능 튜닝

### 비동기 버퍼 크기 설정

**너무 작음:**
- 버퍼가 차면 블로킹 위험
- 비동기 이점 손실

**너무 큼:**
- 과도한 메모리 사용
- 크래시 시 메시지 손실 가능

**권장 크기 설정:**
```rust
// 예상 부하 기반 계산
let logs_per_second = 1000;
let burst_multiplier = 10; // 10배 버스트 처리
let buffer_size = logs_per_second * burst_multiplier;

let logger = Logger::with_async(buffer_size);
```

### 할당 최소화

```rust
// 복잡한 메시지는 format! 사용
logger.info(format!("사용자 {}가 {}에서 로그인", user_id, ip));

// 간단한 메시지는 문자열 리터럴 사용
logger.info("서버 시작됨");

// 불필요한 문자열 작업 피하기
// 나쁨: 중간 문자열 생성
logger.info(format!("{}", format!("사용자: {}", user)));

// 좋음: 단일 format 작업
logger.info(format!("사용자: {}", user));
```

### 조건부 로깅

```rust
// 레벨이 활성화된 경우에만 비용이 많이 드는 작업 수행
if logger.min_level() <= LogLevel::Debug {
    let debug_info = compute_expensive_debug_info();
    logger.debug(format!("디버그 정보: {}", debug_info));
}
```

## 문제 해결

### 로그가 나타나지 않음

**문제:** 로그가 출력에 표시되지 않음

**해결책:**

1. 최소 로그 레벨 확인
```rust
// 레벨이 충분히 낮은지 확인
logger.set_min_level(LogLevel::Trace); // 임시로 모두 활성화
```

2. Appender가 추가되었는지 확인
```rust
// Appender 존재 확인
if logger.appender_count() == 0 {
    logger.add_appender(Box::new(ConsoleAppender::new()));
}
```

3. 비동기 로거 flush
```rust
// 비동기 로거의 경우 flush 보장
logger.flush()?;
```

### 비동기 로거 블로킹

**문제:** 비동기 모드임에도 로거 블로킹

**원인:** 버퍼가 가득 참

**해결책:**

1. 버퍼 크기 증가
```rust
let logger = Logger::with_async(100000); // 더 큰 버퍼
```

2. 로깅 볼륨 감소
```rust
// 최소 레벨 증가
logger.set_min_level(LogLevel::Warn);
```

3. 느린 appender 확인
```rust
// 느린 appender 제거 또는 최적화
```

### 파일 권한 문제

**문제:** 로그 파일에 쓸 수 없음

**해결책:**

1. 디렉토리 권한 확인
```rust
use std::fs;

let log_dir = "/var/log/myapp";
fs::create_dir_all(log_dir)?; // 없으면 생성
```

2. 쓰기 가능한 위치 사용
```rust
// 사용자 디렉토리 또는 temp 사용
use std::env;

let log_path = env::temp_dir().join("app.log");
let file = FileAppender::new(log_path)?;
```

### 메모리 사용량

**문제:** 비동기 로거의 높은 메모리 사용

**해결책:**

1. 버퍼 크기 축소
```rust
let logger = Logger::with_async(1000); // 더 작은 버퍼
```

2. 메시지 크기 감소
```rust
// 긴 메시지 자르기
let message = long_string[..500].to_string();
logger.info(message);
```

### 스레드 안전성 문제

**문제:** 동시 접근 에러

**해결책:** 공유 로거에 Arc 사용
```rust
use std::sync::Arc;

let logger = Arc::new(Logger::with_async(10000));

let logger_clone = Arc::clone(&logger);
std::thread::spawn(move || {
    logger_clone.info("스레드 안전 로깅");
});
```

## Best Practices

### 일반 가이드라인

1. **프로덕션에서 비동기 로거 사용**
   - 더 나은 성능
   - 논블로킹 I/O

2. **적절한 로그 레벨 설정**
   - 개발: Debug
   - 프로덕션: Info
   - 문제 해결: Trace

3. **메시지에 컨텍스트 포함**
   ```rust
   logger.error(format!("주문 {} 처리 실패: {}", order_id, error));
   ```

4. **종료 시 flush**
   ```rust
   // 애플리케이션 종료 전
   logger.flush()?;
   ```

5. **중요한 데이터는 구조화된 로깅 사용**
   ```rust
   logger.info(format!("order_id={} user_id={} amount={}",
       order_id, user_id, amount));
   ```

---

*설정 가이드 버전 1.0*
*최종 업데이트: 2025-10-16*
