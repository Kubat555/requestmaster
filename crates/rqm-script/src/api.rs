//! The table of the Postman sandbox API (`prelude/src/api.json`) and the search of the text of a script for
//! what this engine does not support. The prelude reads the same file for its stubs.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const TABLE: &str = include_str!("../prelude/src/api.json");
const UNSUPPORTED: &str = "unsupported";
/// A chain that begins with these segments is an assertion of chai: any word may follow.
const ASSERTION: [&str; 3] = ["pm", "response", "to"];

struct Table {
    /// Name → "is not supported".
    members: BTreeMap<String, bool>,
    /// Every proper prefix of a name, cut at a dot: `pm.response`, `pm.response.to`, `pm.request.url`.
    namespaces: BTreeSet<String>,
    /// The modules `require` gives.
    modules: BTreeSet<String>,
    /// The most segments a name has.
    depth: usize,
}

/// `(name, status)` of every item of a list of the table.
fn items<'a>(table: &'a Value, list: &str) -> impl Iterator<Item = (&'a str, &'a str)> {
    table
        .get(list)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let name = item.get("name").and_then(Value::as_str)?;
            let status = item.get("status").and_then(Value::as_str)?;
            Some((name, status))
        })
}

impl Table {
    /// A table that does not parse is an empty one; the test `table_is_well_formed` reads the same file.
    fn parse() -> Table {
        let json: Value = serde_json::from_str(TABLE).unwrap_or(Value::Null);
        let mut table = Table {
            members: BTreeMap::new(),
            namespaces: BTreeSet::new(),
            modules: BTreeSet::new(),
            depth: 0,
        };
        for (name, status) in items(&json, "members") {
            table
                .members
                .insert(name.to_string(), status == UNSUPPORTED);
            table.depth = table.depth.max(name.split('.').count());
            for (dot, _) in name.match_indices('.') {
                if let Some(prefix) = name.get(..dot) {
                    table.namespaces.insert(prefix.to_string());
                }
            }
        }
        for (name, status) in items(&json, "modules") {
            if status != UNSUPPORTED {
                table.modules.insert(name.to_string());
            }
        }
        table
    }

    fn get() -> &'static Table {
        static CELL: OnceLock<Table> = OnceLock::new();
        CELL.get_or_init(Table::parse)
    }

    /// What a chain `pm.a.b` or `postman.a` names that is not supported, if anything.
    fn finding(&self, mut segments: Vec<String>) -> Option<String> {
        let assertion = segments.len() >= ASSERTION.len()
            && segments
                .iter()
                .zip(ASSERTION)
                .all(|(have, want)| have == want);
        if assertion {
            // `to.not.have.status` is the assertion `to.have.status`.
            segments.retain(|segment| segment != "not");
        }
        // `pm`, `pm.a`, `pm.a.b`, …: no further than one segment past the longest name — the rest of a
        // chain of any length decides nothing.
        let mut prefixes: Vec<String> = Vec::new();
        let mut name = String::new();
        for segment in segments.iter().take(self.depth.saturating_add(1)) {
            if !name.is_empty() {
                name.push('.');
            }
            name.push_str(segment);
            prefixes.push(name.clone());
        }

        // The longest prefix the table knows decides.
        let known = prefixes
            .iter()
            .rev()
            .find_map(|prefix| self.members.get_key_value(prefix));
        if let Some((member, unsupported)) = known {
            return unsupported.then(|| member.clone());
        }
        let whole_is_namespace = prefixes.len() == segments.len()
            && prefixes
                .last()
                .is_some_and(|whole| self.namespaces.contains(whole));
        if whole_is_namespace || assertion {
            return None;
        }
        prefixes
            .into_iter()
            .find(|prefix| !self.namespaces.contains(prefix))
    }
}

fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == '$'
}

fn is_word_start(character: char) -> bool {
    is_word(character) && !character.is_ascii_digit()
}

/// Where the word that begins at `start` ends.
fn word_end(text: &[char], start: usize) -> usize {
    let mut end = start;
    while text.get(end).copied().is_some_and(is_word) {
        end += 1;
    }
    end
}

fn word(text: &[char], start: usize, end: usize) -> String {
    text.get(start..end)
        .map(|characters| characters.iter().collect())
        .unwrap_or_default()
}

fn skip_spaces(text: &[char], mut at: usize) -> usize {
    while text.get(at).copied().is_some_and(char::is_whitespace) {
        at += 1;
    }
    at
}

/// `('x')` or `("x")` at `at`, spaces allowed around the name: the name and where the call ends.
fn required(text: &[char], at: usize) -> Option<(String, usize)> {
    let open = skip_spaces(text, at);
    if text.get(open) != Some(&'(') {
        return None;
    }
    let quote_at = skip_spaces(text, open + 1);
    let quote = text
        .get(quote_at)
        .copied()
        .filter(|quote| matches!(quote, '\'' | '"'))?;
    let start = quote_at + 1;
    let mut end = start;
    loop {
        match text.get(end).copied() {
            Some(character) if character == quote => break,
            Some('\n' | '\r') | None => return None,
            Some(_) => end += 1,
        }
    }
    let close = skip_spaces(text, end + 1);
    if text.get(close) != Some(&')') {
        return None;
    }
    Some((word(text, start, end), close + 1))
}

/// The names of the Postman sandbox API that the text of a script uses and this engine does not support:
/// sorted, without repeats. A plain text search — comments and strings are searched too.
pub fn unsupported_api(script: &str) -> Vec<String> {
    let table = Table::get();
    let text: Vec<char> = script.chars().collect();
    let mut found = BTreeSet::new();
    let mut at = 0;
    while let Some(first) = text.get(at).copied() {
        if !is_word(first) {
            at += 1;
            continue;
        }
        let start = at;
        at = word_end(&text, start);
        // A member of something else (`x.Buffer`); after `...` a name stands on its own.
        let before = |back: usize| start.checked_sub(back).and_then(|index| text.get(index));
        let is_member = before(1) == Some(&'.') && before(2) != Some(&'.');
        if is_member || !is_word_start(first) {
            continue;
        }
        let name = word(&text, start, at);
        match name.as_str() {
            "pm" | "postman" => {
                let mut segments = vec![name];
                while text.get(at) == Some(&'.')
                    && text.get(at + 1).copied().is_some_and(is_word_start)
                {
                    let end = word_end(&text, at + 1);
                    segments.push(word(&text, at + 1, end));
                    at = end;
                }
                found.extend(table.finding(segments));
            }
            "require" => {
                if let Some((module, end)) = required(&text, at) {
                    at = end;
                    if !table.modules.contains(&module) {
                        found.insert(format!("require('{module}')"));
                    }
                }
            }
            // A global name for later: `CryptoJS`, `Buffer`.
            _ => {
                if table.members.get(&name) == Some(&true) {
                    found.insert(name);
                }
            }
        }
    }
    found.into_iter().collect()
}
