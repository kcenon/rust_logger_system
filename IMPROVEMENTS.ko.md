# Rust Logger System - 개선 계획

> **Languages**: [English](./IMPROVEMENTS.md) | 한국어

## 개요

이 문서는 코드 분석을 바탕으로 식별된 Rust Logger System의 약점과 제안된 개선사항을 설명합니다.

## 식별된 문제점

### 1. 큐 오버플로우 시 로그 손실

**문제**: 비동기 로깅은 내부 채널이 가득 차면 로그를 조용히 버려서, 시스템 과부하나 사고 조사 중에 중요한 진단 정보가 손실될 수 있습니다.

**위치**: `src/logger.rs:234` (async mode)

**현재 구현**:
```rust
pub fn log(&self, record: LogRecord) {
    if self.config.async_mode {
        // 큐가 가득 차면 try_send가 조용히 실패!
        let _ = self.sender.try_send(record);
    } else {
        self.write_log(record);
    }
}
```

**영향**:
- 높은 부하 시 중요한 에러 메시지가 손실될 수 있음
- 로그가 버려졌을 때 알림 없음
- 진단 데이터가 누락되면 문제 디버깅 어려움
- 사고 대응 시 로그 완전성을 신뢰할 수 없음

**제안된 해결책**:

**옵션 1: 오버플로우 정책 설정 추가**

```rust
// TODO: 조용한 로그 손실을 방지하기 위한 설정 가능한 오버플로우 정책 추가

#[derive(Debug, Clone)]
pub enum OverflowPolicy {
    Drop,              // 새 로그 버림 (현재 동작)
    Block,             // 공간이 생길 때까지 차단
    DropOldest,        // 공간을 만들기 위해 가장 오래된 로그 버림
    AlertAndDrop,      // 버리되 운영자에게 알림
}

#[derive(Debug, Clone)]
pub struct LoggerConfig {
    pub async_mode: bool,
    pub queue_size: usize,
    pub overflow_policy: OverflowPolicy,
    pub on_overflow: Option<Box<dyn Fn(usize) + Send + Sync>>,  // 콜백
    // ... 다른 설정
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
                    // 공간이 생길 때까지 차단
                    self.sender.send(record).ok();
                }
                OverflowPolicy::DropOldest => {
                    // 먼저 논블로킹 전송 시도
                    if self.sender.try_send(record.clone()).is_err() {
                        // 큐 가득참, 하나를 빼내고 재시도
                        self.receiver.try_recv().ok();
                        self.metrics.dropped_logs.fetch_add(1, Ordering::Relaxed);
                        self.sender.try_send(record).ok();
                    }
                }
                OverflowPolicy::AlertAndDrop => {
                    if self.sender.try_send(record).is_err() {
                        let dropped = self.metrics.dropped_logs.fetch_add(1, Ordering::Relaxed);

                        // 첫 버림과 1000번째마다 알림
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

**옵션 2: 중요 로그 보존을 위한 우선순위 레벨 추가**

```rust
// TODO: 오버플로우 시 중요 로그를 보존하기 위한 우선순위 시스템 추가

pub enum LogPriority {
    Critical,  // 절대 버리지 않음
    High,      // 최후의 수단으로만 버림
    Normal,    // 압박 시 버릴 수 있음
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
                            // 차단으로 강제 전송 - 중요 로그는 절대 버리지 않음
                            self.sender.send(record).ok();
                        }
                        LogPriority::High => {
                            // 먼저 낮은 우선순위 로그 버리기 시도
                            if self.drop_lowest_priority_log() {
                                self.sender.try_send(record).ok();
                            }
                        }
                        LogPriority::Normal => {
                            // 이 로그 버림
                            self.metrics.dropped_logs.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Err(TrySendError::Disconnected(_)) => {
                    // Logger 종료됨
                }
            }
        } else {
            self.write_log(record);
        }
    }
}
```

**우선순위**: 높음
**예상 작업량**: 중간 (1주)

### 2. 일관되지 않은 타임스탬프 형식

**문제**: 로그 출력의 사용자 정의 타임스탬프 형식이 검증되거나 표준화되지 않아, 파싱 어려움, 일관되지 않은 로그 분석, 로그 집계 시스템과의 통합 문제를 야기합니다.

**위치**: `src/formatter.rs:56`

**현재 구현**:
```rust
pub fn format_timestamp(&self, timestamp: &SystemTime) -> String {
    // 기본 ISO 8601 형식, 설정 불가하거나 표준화되지 않음
    let datetime: DateTime<Utc> = (*timestamp).into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}
```

**영향**:
- 다른 환경에 맞게 타임스탬프 형식 커스터마이징 불가
- 일부 형식에서 시간대 정보 누락
- 자동화된 도구로 로그 파싱 어려움
- 일반적인 로그 집계 시스템(Elasticsearch, Splunk 등)과 호환 불가

**제안된 해결책**:

```rust
// TODO: 표준화되고 설정 가능한 타임스탬프 형식 추가

#[derive(Debug, Clone)]
pub enum TimestampFormat {
    Iso8601,           // 2025-10-17T10:30:45.123Z
    Iso8601WithMicros, // 2025-10-17T10:30:45.123456Z
    Rfc3339,           // 2025-10-17T10:30:45+00:00
    Unix,              // 1697536245
    UnixMillis,        // 1697536245123
    UnixMicros,        // 1697536245123456
    Custom(String),    // 사용자 정의 strftime 형식
}

impl TimestampFormat {
    pub fn format(&self, timestamp: &SystemTime) -> String {
        match self {
            TimestampFormat::Iso8601 => {
                let datetime: DateTime<Utc> = (*timestamp).into();
                datetime.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
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
            // ... 다른 형식들
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormatterConfig {
    pub timestamp_format: TimestampFormat,
    pub include_thread_id: bool,
    pub include_file_location: bool,
    // ... 다른 옵션들
}
```

**우선순위**: 중간
**예상 작업량**: 소 (2-3일)

### 3. 로그 로테이션 전략 없음

**문제**: 파일 appender에 내장된 로그 로테이션이 없어, 무제한 디스크 사용으로 이어질 수 있고 장시간 실행되는 애플리케이션에서 로그 관리가 어렵습니다.

**현재 상태**:
```rust
// src/appenders/file.rs
pub struct FileAppender {
    file: File,
    path: PathBuf,
    // 로테이션 지원 없음!
}
```

**영향**:
- 로그 파일이 무한정 커질 수 있음
- 프로덕션에서 디스크 공간 고갈
- 오래된 로그 관리 및 아카이브 어려움
- 매우 큰 로그 파일로 성능 저하

**제안된 해결책**:

```rust
// TODO: 로그 로테이션 전략 추가

#[derive(Debug, Clone)]
pub enum RotationPolicy {
    Size { max_bytes: u64 },                    // N 바이트 후 로테이션
    Time { interval: Duration },                 // N 시간/일마다 로테이션
    Daily { hour: u8 },                          // 특정 시간에 로테이션
    Hourly,                                      // 매 시간 로테이션
    Hybrid { max_bytes: u64, interval: Duration }, // 크기 또는 시간에 로테이션
}

#[derive(Debug, Clone)]
pub struct FileAppenderConfig {
    pub path: PathBuf,
    pub rotation: Option<RotationPolicy>,
    pub max_backups: usize,                      // N개 오래된 파일 유지
    pub compress_backups: bool,                  // 오래된 파일 Gzip
}

pub struct FileAppender {
    config: FileAppenderConfig,
    current_file: File,
    current_size: u64,
    last_rotation: SystemTime,
}

impl FileAppender {
    pub fn write(&mut self, message: &str) -> std::io::Result<()> {
        // 로테이션 필요한지 확인
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
            // ... 다른 정책들
        }
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        // 현재 파일 닫기
        self.current_file.sync_all()?;

        // 현재 파일을 백업으로 이름 변경
        let backup_path = self.generate_backup_path();
        std::fs::rename(&self.config.path, &backup_path)?;

        // 설정되어 있으면 압축
        if self.config.compress_backups {
            self.compress_file(&backup_path)?;
        }

        // 오래된 백업 정리
        self.cleanup_old_backups()?;

        // 새 파일 열기
        self.current_file = File::create(&self.config.path)?;
        self.current_size = 0;
        self.last_rotation = SystemTime::now();

        Ok(())
    }
}
```

**사용 예제**:
```rust
let appender = FileAppender::with_config(FileAppenderConfig {
    path: PathBuf::from("/var/log/myapp.log"),
    rotation: Some(RotationPolicy::Hybrid {
        max_bytes: 100 * 1024 * 1024,  // 100 MB
        interval: Duration::from_secs(24 * 3600),  // 24시간
    }),
    max_backups: 7,  // 일주일치 로그 보관
    compress_backups: true,
});
```

**우선순위**: 중간
**예상 작업량**: 중간 (1주)

## 추가 개선사항

자세한 내용은 [영문 버전](./IMPROVEMENTS.md)의 추가 개선사항 섹션을 참조하세요:

- 컨텍스트 및 구조화된 필드
- 대용량 로그를 위한 샘플링

## 테스트 요구사항

### 필요한 새 테스트:

1. **오버플로우 정책 테스트**:
   ```rust
   #[test]
   fn test_drop_oldest_policy() {
       let logger = Logger::with_config(LoggerConfig {
           async_mode: true,
           queue_size: 10,
           overflow_policy: OverflowPolicy::DropOldest,
           ..Default::default()
       });

       // 큐를 채움
       for i in 0..15 {
           logger.info(&format!("Message {}", i));
       }

       // 처리 대기
       logger.flush();

       // 가장 오래된 5개 메시지를 버렸어야 함
       assert_eq!(logger.dropped_log_count(), 5);
   }
   ```

2. **타임스탬프 형식 테스트**:
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

           // 파싱 가능한지 검증
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

3. **로그 로테이션 테스트**:
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

       // 5 KB 로그 작성
       for _ in 0..50 {
           appender.write(&"x".repeat(100)).unwrap();
       }

       // 로테이션되고 백업 파일이 생성되었어야 함
       let backups: Vec<_> = std::fs::read_dir(temp_dir.path())
           .unwrap()
           .filter_map(|e| e.ok())
           .filter(|e| e.file_name() != "test.log")
           .collect();

       assert!(backups.len() >= 3);
   }
   ```

## 구현 로드맵

### 1단계: 중요 신뢰성 (스프린트 1)
- [ ] 오버플로우 정책 구현
- [ ] 버려진 로그 메트릭 추가
- [ ] 오버플로우 알림 추가
- [ ] 모든 오버플로우 시나리오 테스트

### 2단계: 형식 및 표준 (스프린트 2)
- [ ] 표준 타임스탬프 형식 추가
- [ ] JSON 출력 형식 구현
- [ ] 구조화된 로깅 지원 추가
- [ ] 문서 업데이트

### 3단계: 프로덕션 기능 (스프린트 3)
- [ ] 로그 로테이션 구현
- [ ] 압축 지원 추가
- [ ] 로테이션 테스트 생성
- [ ] 운영 가이드 추가

### 4단계: 고급 기능 (스프린트 4)
- [ ] 로그 샘플링 추가
- [ ] 컨텍스트 전파 구현
- [ ] 성능 벤치마크 추가
- [ ] 고급 예제 생성

## Breaking Changes

⚠️ **주의**: 기본 오버플로우 동작 변경은 기존 배포에 영향을 줄 수 있습니다.

**마이그레이션 경로**:
1. 버전 1.x: 현재 동작을 기본값으로 새 오버플로우 정책 추가
2. 버전 1.x: 조용한 버림에 대한 deprecated 경고 추가
3. 버전 2.0: 기본값을 AlertAndDrop 정책으로 변경
4. CHANGELOG에 마이그레이션 문서화

## 성능 목표

### 현재 성능:
- 동기 로깅: ~100k logs/sec
- 비동기 로깅: ~500k logs/sec
- 로그 손실: 알 수 없음 (추적 안 됨)

### 개선 후 목표 성능:
- 동기 로깅: ~100k logs/sec (변경 없음)
- 비동기 로깅: ~500k logs/sec (변경 없음)
- 로그 손실: 중요 로그 0%, 일반 로그 <0.1%
- 버려진 로그 추적: 100% 정확
- 로테이션 오버헤드: 로테이션당 <1ms

## 참고자료

- 코드 분석: Logger System Review 2025-10-16
- 관련 이슈:
  - 로그 손실 (#TODO)
  - 타임스탬프 형식 (#TODO)
  - 로그 로테이션 (#TODO)

---

*개선 계획 버전 1.0*
*최종 업데이트: 2025-10-17*
