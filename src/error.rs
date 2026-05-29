//! Runtime error types.

use std::fmt;

/// Location in source: file (optional), 1-based line and column.
#[derive(Debug, Clone, Default)]
pub struct SourceLoc {
    pub file: Option<String>,
    pub line: u32,
    pub column: u32,
    pub offset: usize,
}

impl SourceLoc {
    pub fn new(file: Option<String>, line: u32, column: u32, offset: usize) -> Self {
        Self { file, line, column, offset }
    }
}

/// Parse error with location and optional source snippet for UI display.
#[derive(Debug, Clone)]
pub struct ParseErrorDetail {
    pub message: String,
    pub loc: SourceLoc,
    /// The full source line containing the error (for highlighting).
    pub source_line: Option<String>,
}

impl ParseErrorDetail {
    /// Format for display. Includes file:line:column and message.
    pub fn display(&self) -> String {
        let mut out = String::new();
        if let Some(ref f) = self.loc.file {
            out.push_str(f);
            out.push(':');
        }
        if self.loc.line > 0 {
            out.push_str(&self.loc.line.to_string());
            out.push(':');
            out.push_str(&self.loc.column.to_string());
            if !out.ends_with(':') {
                out.push_str(": ");
            }
        }
        out.push_str(&self.message);
        if let Some(ref line) = self.source_line {
            out.push_str("\n  ");
            out.push_str(line);
            out.push('\n');
            out.push_str(&"  ".to_string());
            out.push_str(&" ".repeat((self.loc.column.saturating_sub(1)) as usize));
            out.push_str("^");
        }
        out
    }

    /// JSON-serializable for error UI (file, line, column, message, source_line).
    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert("message".to_string(), serde_json::Value::String(self.message.clone()));
        obj.insert("line".to_string(), serde_json::Value::Number(serde_json::Number::from(self.loc.line)));
        obj.insert("column".to_string(), serde_json::Value::Number(serde_json::Number::from(self.loc.column)));
        obj.insert("offset".to_string(), serde_json::Value::Number(serde_json::Number::from(self.loc.offset as u64)));
        if let Some(ref f) = self.loc.file {
            obj.insert("file".to_string(), serde_json::Value::String(f.clone()));
        }
        if let Some(ref s) = self.source_line {
            obj.insert("source_line".to_string(), serde_json::Value::String(s.clone()));
        }
        serde_json::Value::Object(obj)
    }
}

/// Runtime error with call stack and locals for debugging (Tier 1).
#[derive(Debug, Clone)]
pub struct RuntimeErrorDetail {
    pub message: String,
    pub call_stack: Vec<String>,
    pub locals: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub enum RuntimeError {
    Parse(String),
    ParseDetail(ParseErrorDetail),
    Ir(String),
    Runtime(String),
    /// Runtime error with call stack and locals snapshot for host debug UI.
    RuntimeDetail(RuntimeErrorDetail),
    Host(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::Parse(s) => write!(f, "Parse error: {}", s),
            RuntimeError::ParseDetail(d) => write!(f, "Parse error: {}", d.display()),
            RuntimeError::Ir(s) => write!(f, "IR error: {}", s),
            RuntimeError::Runtime(s) => write!(f, "Runtime error: {}", s),
            RuntimeError::RuntimeDetail(d) => {
                write!(f, "Runtime error: {}", d.message)?;
                if !d.call_stack.is_empty() {
                    write!(f, " (at {})", d.call_stack.join(" → "))?;
                }
                Ok(())
            }
            RuntimeError::Host(s) => write!(f, "Host error: {}", s),
        }
    }
}

impl std::error::Error for RuntimeError {}

/// JSON for host error UI. Contract: { type, message, file?, line?, column?, source_line? }
impl RuntimeError {
    pub fn to_error_json(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        match self {
            RuntimeError::Parse(s) => {
                obj.insert("type".to_string(), serde_json::json!("parse"));
                obj.insert("message".to_string(), serde_json::Value::String(s.clone()));
            }
            RuntimeError::ParseDetail(d) => {
                obj.insert("type".to_string(), serde_json::json!("parse"));
                obj.insert("message".to_string(), serde_json::Value::String(d.message.clone()));
                if let Some(ref f) = d.loc.file {
                    obj.insert("file".to_string(), serde_json::Value::String(f.clone()));
                }
                obj.insert("line".to_string(), serde_json::json!(d.loc.line));
                obj.insert("column".to_string(), serde_json::json!(d.loc.column));
                if let Some(ref s) = d.source_line {
                    obj.insert("source_line".to_string(), serde_json::Value::String(s.clone()));
                }
            }
            RuntimeError::Ir(s) | RuntimeError::Runtime(s) | RuntimeError::Host(s) => {
                obj.insert("type".to_string(), serde_json::json!("runtime"));
                obj.insert("message".to_string(), serde_json::Value::String(s.clone()));
            }
            RuntimeError::RuntimeDetail(d) => {
                obj.insert("type".to_string(), serde_json::json!("runtime"));
                obj.insert("message".to_string(), serde_json::Value::String(d.message.clone()));
                obj.insert(
                    "call_stack".to_string(),
                    serde_json::Value::Array(
                        d.call_stack
                            .iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect(),
                    ),
                );
                obj.insert("locals".to_string(), serde_json::Value::Object(d.locals.clone()));
            }
        }
        serde_json::Value::Object(obj)
    }
}

/// Convert character offset (0-based) to 1-based (line, column).
pub fn line_col_for_offset(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;
    let mut i = 0usize;
    for c in source.chars() {
        if i >= offset {
            return (line, col);
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
        i += 1;
    }
    (line, col)
}

/// Get the 1-based line content at the given line number.
pub fn get_line_at(source: &str, line_num: u32) -> Option<String> {
    let mut current = 1u32;
    for line in source.lines() {
        if current == line_num {
            return Some(line.to_string());
        }
        current += 1;
    }
    None
}
