//! What the response viewer shows of a body stored on disk.

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Inline display limit from the spec, section 4: 20 MiB.
pub const INLINE_BODY_LIMIT: u64 = 20 * 1024 * 1024;
/// How much of a larger body is shown: 256 KiB.
pub const TRUNCATED_PREVIEW: u64 = 256 * 1024;

/// UTF-8 byte order mark; some servers put it before the JSON, and a JSON parser does not accept it.
const BOM: &[u8] = b"\xEF\xBB\xBF";

/// The part of a response body to display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyView {
    /// The whole body, its re-indented form, or the first [`TRUNCATED_PREVIEW`] bytes of it.
    pub bytes: Vec<u8>,
    /// Size of the body file.
    pub total: u64,
    /// `true` when the body is larger than [`INLINE_BODY_LIMIT`] and `bytes` is only its beginning.
    pub truncated: bool,
    /// `true` when `bytes` is the re-indented JSON and not the body as stored.
    pub pretty: bool,
}

/// Whole body when `total <= INLINE_BODY_LIMIT`, otherwise the first `TRUNCATED_PREVIEW` bytes.
/// `pretty = true` re-indents JSON (2 spaces, key order kept) when the body is valid JSON and not truncated.
/// One leading UTF-8 byte order mark does not prevent that and is left out of the re-indented text; the body
/// as stored (`pretty = false`, or not JSON) is returned with it.
pub fn body_view(path: &Path, pretty: bool) -> std::io::Result<BodyView> {
    let file = File::open(path)?;
    let total = file.metadata()?.len();
    let truncated = total > INLINE_BODY_LIMIT;
    let shown = if truncated { TRUNCATED_PREVIEW } else { total };
    // The size is a hint only: the cast cannot matter on a 64-bit target and a wrong hint is harmless.
    let mut bytes = Vec::with_capacity(shown as usize);
    file.take(shown).read_to_end(&mut bytes)?;
    let json = bytes.strip_prefix(BOM).unwrap_or(&bytes);
    let pretty = pretty && !truncated && is_json(json);
    if pretty {
        bytes = reindent(json);
    }
    Ok(BodyView { bytes, total, truncated, pretty })
}

/// Re-indents JSON text the way the response viewer does: 2 spaces, every number, key order and duplicate
/// key exactly as written. One leading byte order mark is dropped. `None` when the text is not valid JSON.
pub fn format_json(text: &str) -> Option<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if !is_json(text.as_bytes()) {
        return None;
    }
    // `reindent` changes only ASCII whitespace between tokens, so the text stays valid UTF-8.
    String::from_utf8(reindent(text.as_bytes())).ok()
}

/// Whether the bytes are one valid JSON value.
fn is_json(bytes: &[u8]) -> bool {
    // `IgnoredAny` checks the syntax without building a value, so numbers and key order are never altered.
    serde_json::from_slice::<serde::de::IgnoredAny>(bytes).is_ok()
}

/// Re-indents valid JSON with 2 spaces. Works on the text: every token (numbers, strings, duplicate keys) is
/// copied as written, only the whitespace between tokens changes. Empty `{}` and `[]` stay on one line.
fn reindent(json: &[u8]) -> Vec<u8> {
    fn new_line(out: &mut Vec<u8>, depth: usize) {
        out.push(b'\n');
        out.resize(out.len() + depth * 2, b' ');
    }
    let mut out = Vec::with_capacity(json.len() * 2);
    let mut depth = 0usize;
    let mut i = 0;
    while i < json.len() {
        let byte = json[i];
        i += 1;
        match byte {
            b'"' => {
                // Copy the string up to its closing quote; a backslash takes the next byte with it.
                out.push(byte);
                while i < json.len() {
                    let inner = json[i];
                    out.push(inner);
                    i += 1;
                    if inner == b'\\' && i < json.len() {
                        out.push(json[i]);
                        i += 1;
                    } else if inner == b'"' {
                        break;
                    }
                }
            }
            b' ' | b'\t' | b'\n' | b'\r' => {}
            b'{' | b'[' => {
                out.push(byte);
                let next = json[i..].iter().position(|b| !b.is_ascii_whitespace()).map(|offset| i + offset);
                match next {
                    Some(at) if matches!(json[at], b'}' | b']') => {
                        out.push(json[at]);
                        i = at + 1;
                    }
                    _ => {
                        depth += 1;
                        new_line(&mut out, depth);
                    }
                }
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                new_line(&mut out, depth);
                out.push(byte);
            }
            b',' => {
                out.push(byte);
                new_line(&mut out, depth);
            }
            b':' => out.extend_from_slice(b": "),
            _ => out.push(byte),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{format_json, reindent};

    fn pretty(json: &str) -> String {
        String::from_utf8(reindent(json.as_bytes())).unwrap()
    }

    #[test]
    fn tokens_are_copied_as_written() {
        assert_eq!(pretty(" {\"b\" : 1.50 ,\"a\":[ ] , \"o\":{ }}\n"), "{\n  \"b\": 1.50,\n  \"a\": [],\n  \"o\": {}\n}");
        assert_eq!(pretty("{\"s\":\"a,\\\"{:}\\\\\",\"n\":12345678901234567890123}"), "{\n  \"s\": \"a,\\\"{:}\\\\\",\n  \"n\": 12345678901234567890123\n}");
        assert_eq!(pretty("[[1],null]"), "[\n  [\n    1\n  ],\n  null\n]");
        assert_eq!(pretty("7"), "7");
    }

    #[test]
    fn format_json_reindents_valid_json_only() {
        assert_eq!(format_json(r#"{"b":1.50,"a":[1,2]}"#).as_deref(), Some("{\n  \"b\": 1.50,\n  \"a\": [\n    1,\n    2\n  ]\n}"));
        // numbers, key order and duplicate keys stay as written
        assert_eq!(
            format_json(r#"{"z":12345678901234567890123,"a":1e2,"z":0.10}"#).as_deref(),
            Some("{\n  \"z\": 12345678901234567890123,\n  \"a\": 1e2,\n  \"z\": 0.10\n}")
        );
        assert_eq!(format_json(r#"{"имя":"значение"}"#).as_deref(), Some("{\n  \"имя\": \"значение\"\n}"));
        // a leading byte order mark is dropped
        assert_eq!(format_json("\u{feff}[1]").as_deref(), Some("[\n  1\n]"));
    }

    #[test]
    fn format_json_refuses_what_is_not_json() {
        assert_eq!(format_json(""), None);
        assert_eq!(format_json("not json"), None);
        assert_eq!(format_json(r#"{"a":1,}"#), None);
        // a variable outside quotes is not JSON until it is substituted
        assert_eq!(format_json(r#"{"id": {{id}}}"#), None);
        assert_eq!(format_json("\u{feff}\u{feff}[1]"), None, "only one byte order mark is skipped");
    }
}
