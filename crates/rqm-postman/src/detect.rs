//! Reading the text of a file and telling which Postman export it is.

use serde_json::error::Category;
use serde_json::Value;

/// Kind of a Postman export file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detected {
    /// A collection of schema v2.1.0 (`info.schema`).
    Collection,
    /// An environment (`_postman_variable_scope: "environment"`).
    Environment,
    /// The globals set (`_postman_variable_scope: "globals"`).
    Globals,
}

/// Why a file cannot be imported.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    /// The text is not JSON. Carries only the parser message (kind, line, column),
    /// never a fragment of the input.
    #[error("the file is not valid JSON: {0}")]
    NotJson(String),
    /// JSON, but not a Postman collection, environment or globals export.
    #[error("the file is not a Postman collection, environment or globals export")]
    UnknownFormat,
    /// A collection of a schema version that is not supported. Carries only the version.
    #[error("Postman collection schema {0} is not supported, export the collection as v2.1")]
    UnsupportedSchema(String),
}

/// Parses the text of a file. Strips a UTF-8 BOM, then parses.
pub fn parse_text(text: &str) -> Result<Value, ImportError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    serde_json::from_str(text).map_err(|e| {
        // The message is built from the error category and position only,
        // so that nothing from the file can leak into logs or the interface.
        let kind = match e.classify() {
            Category::Syntax => "syntax error",
            Category::Eof => "unexpected end of input",
            Category::Data => "data error",
            Category::Io => "read error",
        };
        ImportError::NotJson(format!("{kind} at line {} column {}", e.line(), e.column()))
    })
}

/// Tells which Postman export the JSON is: by `info.schema` for collections
/// and by `_postman_variable_scope` for environments and globals.
pub fn detect(json: &Value) -> Result<Detected, ImportError> {
    if let Some(schema) = json.pointer("/info/schema").and_then(Value::as_str) {
        if schema.contains("/v2.1.0/") {
            return Ok(Detected::Collection);
        }
        if schema.contains("/v2.0.0/") {
            return Err(ImportError::UnsupportedSchema("v2.0.0".to_string()));
        }
    }
    match json.get("_postman_variable_scope").and_then(Value::as_str) {
        Some("environment") => Ok(Detected::Environment),
        Some("globals") => Ok(Detected::Globals),
        _ => Err(ImportError::UnknownFormat),
    }
}
