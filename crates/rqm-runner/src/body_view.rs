//! What the response viewer shows of a body stored on disk.

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Inline display limit from the spec, section 4: 20 MiB.
pub const INLINE_BODY_LIMIT: u64 = 20 * 1024 * 1024;
/// How much of a larger body is shown: 256 KiB.
pub const TRUNCATED_PREVIEW: u64 = 256 * 1024;

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
pub fn body_view(path: &Path, pretty: bool) -> std::io::Result<BodyView> {
    let file = File::open(path)?;
    let total = file.metadata()?.len();
    let truncated = total > INLINE_BODY_LIMIT;
    let shown = if truncated { TRUNCATED_PREVIEW } else { total };
    // The size is a hint only: the cast cannot matter on a 64-bit target and a wrong hint is harmless.
    let mut bytes = Vec::with_capacity(shown as usize);
    file.take(shown).read_to_end(&mut bytes)?;
    // `IgnoredAny` checks the syntax without building a value, so numbers and key order are never altered.
    let pretty = pretty && !truncated && serde_json::from_slice::<serde::de::IgnoredAny>(&bytes).is_ok();
    if pretty {
        bytes = reindent(&bytes);
    }
    Ok(BodyView { bytes, total, truncated, pretty })
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
    use super::reindent;

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
}
