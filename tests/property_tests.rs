//! Property-based tests for rust_logger_system using proptest

use proptest::prelude::*;
use rust_logger_system::prelude::*;

// ============================================================================
// LogLevel Tests
// ============================================================================

proptest! {
    /// Test that LogLevel string conversions roundtrip correctly
    #[test]
    fn test_log_level_str_roundtrip(level in prop_oneof![
        Just(LogLevel::Trace),
        Just(LogLevel::Debug),
        Just(LogLevel::Info),
        Just(LogLevel::Warn),
        Just(LogLevel::Error),
        Just(LogLevel::Fatal),
    ]) {
        let as_str = level.to_str();
        let parsed: LogLevel = as_str.parse().unwrap();
        assert_eq!(level, parsed);
    }

    /// Test that LogLevel ordering is consistent
    #[test]
    fn test_log_level_ordering(
        level1 in prop_oneof![
            Just(LogLevel::Trace),
            Just(LogLevel::Debug),
            Just(LogLevel::Info),
            Just(LogLevel::Warn),
            Just(LogLevel::Error),
            Just(LogLevel::Fatal),
        ],
        level2 in prop_oneof![
            Just(LogLevel::Trace),
            Just(LogLevel::Debug),
            Just(LogLevel::Info),
            Just(LogLevel::Warn),
            Just(LogLevel::Error),
            Just(LogLevel::Fatal),
        ]
    ) {
        let val1 = level1 as u8;
        let val2 = level2 as u8;

        assert_eq!(level1 <= level2, val1 <= val2);
        assert_eq!(level1 < level2, val1 < val2);
        assert_eq!(level1 >= level2, val1 >= val2);
        assert_eq!(level1 > level2, val1 > val2);
    }

    /// Test that LogLevel Display matches to_str
    #[test]
    fn test_log_level_display(level in prop_oneof![
        Just(LogLevel::Trace),
        Just(LogLevel::Debug),
        Just(LogLevel::Info),
        Just(LogLevel::Warn),
        Just(LogLevel::Error),
        Just(LogLevel::Fatal),
    ]) {
        assert_eq!(format!("{}", level), level.to_str());
    }

    /// Test that parsing accepts case-insensitive input
    #[test]
    fn test_log_level_case_insensitive(use_lower in any::<bool>()) {
        let levels = vec!["TRACE", "DEBUG", "INFO", "WARN", "ERROR", "FATAL"];

        for level_str in levels {
            let input = if use_lower {
                level_str.to_lowercase()
            } else {
                level_str.to_string()
            };

            let parsed: std::result::Result<LogLevel, String> = input.parse();
            assert!(parsed.is_ok(), "Failed to parse: {}", input);
        }
    }
}

// ============================================================================
// LogEntry Message Sanitization Tests (Security Critical!)
// ============================================================================

proptest! {
    /// Test that newlines are sanitized in log messages (prevents log injection)
    #[test]
    fn test_message_sanitization_newlines(message in ".*") {
        let entry = LogEntry::new(LogLevel::Info, message.clone());

        // Message should not contain actual newlines
        assert!(!entry.message.contains('\n'),
                "LogEntry contains unsanitized newline: {:?}", entry.message);

        // If original had newlines, they should be escaped
        if message.contains('\n') {
            assert!(entry.message.contains("\\n"),
                    "Newlines not properly escaped: {:?}", entry.message);
        }
    }

    /// Test that carriage returns are sanitized (prevents log injection)
    #[test]
    fn test_message_sanitization_carriage_return(message in ".*") {
        let entry = LogEntry::new(LogLevel::Info, message.clone());

        // Message should not contain actual carriage returns
        assert!(!entry.message.contains('\r'),
                "LogEntry contains unsanitized carriage return: {:?}", entry.message);

        // If original had carriage returns, they should be escaped
        if message.contains('\r') {
            assert!(entry.message.contains("\\r"),
                    "Carriage returns not properly escaped: {:?}", entry.message);
        }
    }

    /// Test that tabs are sanitized
    #[test]
    fn test_message_sanitization_tabs(message in ".*") {
        let entry = LogEntry::new(LogLevel::Info, message.clone());

        // Message should not contain actual tabs
        assert!(!entry.message.contains('\t'),
                "LogEntry contains unsanitized tab: {:?}", entry.message);

        // If original had tabs, they should be escaped
        if message.contains('\t') {
            assert!(entry.message.contains("\\t"),
                    "Tabs not properly escaped: {:?}", entry.message);
        }
    }

    /// Test that log injection attacks are prevented
    #[test]
    fn test_log_injection_prevention(
        legitimate_msg in "[a-zA-Z0-9 ]+",
        injected_level in prop_oneof![
            Just("ERROR"),
            Just("WARN"),
            Just("FATAL"),
        ]
    ) {
        // Simulate an attacker trying to inject a fake log entry
        let malicious_input = format!("{}\n{}: Fake admin login", legitimate_msg, injected_level);
        let entry = LogEntry::new(LogLevel::Info, malicious_input);

        // The sanitized message should not allow a fake entry on a new line
        let lines: Vec<&str> = entry.message.split('\n').collect();
        assert_eq!(lines.len(), 1,
                   "Message was not properly sanitized, contains multiple lines: {:?}",
                   entry.message);
    }
}

// ============================================================================
// LogEntry Tests
// ============================================================================

proptest! {
    /// Test that LogEntry with_location works correctly
    #[test]
    fn test_log_entry_with_location(
        message in ".*",
        file in "[a-z]+\\.rs",
        line in 1u32..10000u32,
        module in "[a-z_:]+"
    ) {
        let entry = LogEntry::new(LogLevel::Info, message)
            .with_location(&file, line, &module);

        assert_eq!(entry.file, Some(file));
        assert_eq!(entry.line, Some(line));
        assert_eq!(entry.module_path, Some(module));
    }

    /// Test that LogEntry always has a timestamp
    #[test]
    fn test_log_entry_has_timestamp(message in ".*") {
        let entry = LogEntry::new(LogLevel::Info, message);

        // Timestamp should be recent (within last second)
        let now = chrono::Utc::now();
        let age = now.signed_duration_since(entry.timestamp);

        assert!(age.num_seconds() <= 1,
                "Timestamp too old: {:?}", entry.timestamp);
    }

    /// Test that LogEntry has thread information
    #[test]
    fn test_log_entry_thread_info(message in ".*") {
        let entry = LogEntry::new(LogLevel::Info, message);

        // Should always have a thread_id (even if name is None)
        assert!(!entry.thread_id.is_empty());
    }

    /// Test that LogEntry cloning works correctly
    #[test]
    fn test_log_entry_clone(message in ".*") {
        let original = LogEntry::new(LogLevel::Error, message.clone());
        let cloned = original.clone();

        assert_eq!(original.level, cloned.level);
        assert_eq!(original.message, cloned.message);
        assert_eq!(original.timestamp, cloned.timestamp);
        assert_eq!(original.thread_id, cloned.thread_id);
    }
}

// ============================================================================
// JSON Serialization Tests
// ============================================================================

proptest! {
    /// Test that LogEntry JSON serialization never panics
    #[test]
    fn test_log_entry_json_serialization(
        message in ".*",
        level in prop_oneof![
            Just(LogLevel::Trace),
            Just(LogLevel::Debug),
            Just(LogLevel::Info),
            Just(LogLevel::Warn),
            Just(LogLevel::Error),
            Just(LogLevel::Fatal),
        ]
    ) {
        let entry = LogEntry::new(level, message);
        let json_result = serde_json::to_string(&entry);

        assert!(json_result.is_ok(), "Failed to serialize LogEntry: {:?}", json_result.err());

        // Verify it can be deserialized back
        if let Ok(json_str) = json_result {
            let deserialized: serde_json::Result<LogEntry> = serde_json::from_str(&json_str);
            assert!(deserialized.is_ok(), "Failed to deserialize LogEntry");
        }
    }

    /// Test that LogLevel JSON serialization never panics
    #[test]
    fn test_log_level_json_serialization(level in prop_oneof![
        Just(LogLevel::Trace),
        Just(LogLevel::Debug),
        Just(LogLevel::Info),
        Just(LogLevel::Warn),
        Just(LogLevel::Error),
        Just(LogLevel::Fatal),
    ]) {
        let json_result = serde_json::to_string(&level);
        assert!(json_result.is_ok());

        // Verify roundtrip
        if let Ok(json_str) = json_result {
            let deserialized: serde_json::Result<LogLevel> = serde_json::from_str(&json_str);
            assert!(deserialized.is_ok());
            assert_eq!(deserialized.unwrap(), level);
        }
    }
}

// ============================================================================
// Safety Tests (No Panics)
// ============================================================================

proptest! {
    /// Test that LogEntry creation never panics
    #[test]
    fn test_log_entry_no_panic(
        message in ".*",
        level in prop_oneof![
            Just(LogLevel::Trace),
            Just(LogLevel::Debug),
            Just(LogLevel::Info),
            Just(LogLevel::Warn),
            Just(LogLevel::Error),
            Just(LogLevel::Fatal),
        ]
    ) {
        // Should never panic regardless of input
        let _ = LogEntry::new(level, message);
    }

    /// Test that FromStr for LogLevel handles invalid input gracefully
    #[test]
    fn test_log_level_invalid_parse(invalid_str in "[^TDIWEFtdiwefor]+") {
        let result: std::result::Result<LogLevel, String> = invalid_str.parse();

        // Should return Err, not panic
        if !invalid_str.is_empty() {
            assert!(result.is_err(),
                    "Expected parse error for '{}', got: {:?}", invalid_str, result);
        }
    }
}

// ============================================================================
// Appender Integration Tests
// ============================================================================

proptest! {
    /// Test that ConsoleAppender handles various log entries without panic
    #[test]
    fn test_console_appender_no_panic(
        messages in prop::collection::vec(".*", 0..10)
    ) {
        let mut appender = ConsoleAppender::new();

        for message in messages {
            let entry = LogEntry::new(LogLevel::Info, message);
            // Should not panic
            let result = appender.append(&entry);
            assert!(result.is_ok(), "ConsoleAppender failed: {:?}", result);
        }
    }
}
