//! File appender implementation

use crate::core::{Appender, LogEntry, LoggerError, Result};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

pub struct FileAppender {
    writer: Option<BufWriter<File>>,
}

impl FileAppender {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let writer = Some(BufWriter::new(file));

        Ok(Self { writer })
    }
}

impl Appender for FileAppender {
    fn append(&mut self, entry: &LogEntry) -> Result<()> {
        let writer = self.writer.as_mut()
            .ok_or_else(|| LoggerError::writer("File writer not initialized"))?;

        let mut output = format!(
            "[{}] [{:5}] [{}] {}",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
            entry.level.to_str(),
            entry.thread_name.as_ref().unwrap_or(&entry.thread_id),
            entry.message
        );

        // Append context fields if present
        if let Some(ref context) = entry.context {
            output.push_str(" | ");
            output.push_str(&context.to_string());
        }

        output.push('\n');

        writer.write_all(output.as_bytes())?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if let Some(ref mut writer) = self.writer {
            writer.flush()?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "file"
    }
}

impl Drop for FileAppender {
    fn drop(&mut self) {
        // Ensure all buffered data is flushed to disk
        let _ = self.flush();
    }
}
