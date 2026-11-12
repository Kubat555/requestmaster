//! RequestMaster variables: scopes, `{{name}}` substitution, `/:name` path variables and dynamic values.
//!
//! The substitution algorithm follows postman-collection (`lib/superstring`): a token is `{{name}}` where the
//! name contains neither `{` nor `}`; the innermost tokens are replaced left to right, pass after pass.

mod dynamic;

use std::collections::{HashMap, HashSet};

pub use dynamic::system_dynamic;

/// Upper bound of substitution passes, the same as in postman-collection. It is what makes
/// self-referencing and cyclic variables terminate.
const MAX_PASSES: usize = 19;

/// Kind of a variable scope, from the widest (`Global`) to the narrowest (`Local`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Globals of the workspace.
    Global,
    /// Variables of the collection the request belongs to.
    Collection,
    /// Variables of the active environment.
    Environment,
    /// Variables that live for one request run.
    Local,
}

/// Variables of one scope, prepared by the caller.
#[derive(Debug, Clone)]
pub struct Scope {
    /// Which scope the values come from; reported back by [`Resolver::lookup`].
    pub kind: ScopeKind,
    /// `(name, value)` pairs of enabled variables only, in their original order.
    pub values: Vec<(String, String)>,
}

/// Result of a substitution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Substituted {
    /// The input with every resolvable token replaced.
    pub text: String,
    /// Names of tokens that were not found (without braces), deduplicated, in order of first appearance.
    pub unresolved: Vec<String>,
}

/// Resolves variable names against a stack of scopes and substitutes `{{name}}` tokens in text.
#[derive(Debug, Clone)]
pub struct Resolver {
    /// Effective value and its scope for every name, after the "narrower scope wins" rule.
    values: HashMap<String, (String, ScopeKind)>,
}

impl Resolver {
    /// Scopes from the widest to the narrowest; a narrower scope wins. Within one scope the last entry wins.
    pub fn new(scopes: Vec<Scope>) -> Self {
        let mut values = HashMap::new();
        for scope in scopes {
            for (name, value) in scope.values {
                values.insert(name, (value, scope.kind));
            }
        }
        Self { values }
    }

    /// Effective value of the variable and the scope it comes from.
    pub fn lookup(&self, name: &str) -> Option<(&str, ScopeKind)> {
        self.values.get(name).map(|(value, kind)| (value.as_str(), *kind))
    }

    /// Uses the system clock and random generator for `{{$...}}`.
    pub fn substitute(&self, input: &str) -> Substituted {
        self.substitute_with(input, &mut system_dynamic)
    }

    /// `dynamic(name)` is called once per occurrence of `{{$name}}`; `None` leaves the token as is.
    pub fn substitute_with(&self, input: &str, dynamic: &mut dyn FnMut(&str) -> Option<String>) -> Substituted {
        let mut text = input.to_string();
        let mut unresolved = Vec::new();
        // Names already known as not found: such a token stays in the text and is met again on every
        // later pass, but must be neither reported nor passed to `dynamic` twice.
        let mut seen_unresolved = HashSet::new();
        for _ in 0..MAX_PASSES {
            let next = self.pass(&text, dynamic, &mut unresolved, &mut seen_unresolved);
            if next == text {
                break;
            }
            text = next;
        }
        Substituted { text, unresolved }
    }

    /// One pass: replaces every innermost token of `text`, left to right.
    fn pass(
        &self,
        text: &str,
        dynamic: &mut dyn FnMut(&str) -> Option<String>,
        unresolved: &mut Vec<String>,
        seen_unresolved: &mut HashSet<String>,
    ) -> String {
        // Braces are ASCII, so every index used for slicing below is a UTF-8 character boundary.
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut copied = 0; // everything before this index is already in `out`
        let mut i = 0;
        while i + 1 < bytes.len() {
            if bytes[i] != b'{' || bytes[i + 1] != b'{' {
                i += 1;
                continue;
            }
            let name_start = i + 2;
            let name_end = bytes[name_start..]
                .iter()
                .position(|b| *b == b'{' || *b == b'}')
                .map_or(bytes.len(), |offset| name_start + offset);
            let closed = bytes.get(name_end) == Some(&b'}') && bytes.get(name_end + 1) == Some(&b'}');
            if !closed {
                // Not a token at this position; an inner `{{` may still start one.
                i += 1;
                continue;
            }
            let name = &text[name_start..name_end];
            let token_end = name_end + 2;
            if !seen_unresolved.contains(name) {
                let value = match self.values.get(name) {
                    Some((value, _)) => Some(value.clone()),
                    None => name.strip_prefix('$').and_then(&mut *dynamic),
                };
                match value {
                    Some(value) => {
                        out.push_str(&text[copied..i]);
                        out.push_str(&value);
                        copied = token_end;
                    }
                    None => {
                        seen_unresolved.insert(name.to_string());
                        unresolved.push(name.to_string());
                    }
                }
            }
            i = token_end;
        }
        out.push_str(&text[copied..]);
        out
    }
}

/// Replaces `/:name` path segments with enabled path-variable values. The name runs from `:` to the first `.`
/// or to the end of the segment (next `/`, `?` or `#`); the tail after the dot is kept (`/:id.json` -> `/7.json`).
/// A segment without a matching variable, or whose variable has an empty value, stays as is.
pub fn apply_path_variables(url: &str, vars: &[(String, String)]) -> String {
    // The query and the fragment are not touched. The delimiters are ASCII, so slicing is safe.
    let path_end = url.find(['?', '#']).unwrap_or(url.len());
    let path = &url[..path_end];
    let mut out = String::with_capacity(url.len());
    let mut rest = path;
    while let Some(at) = rest.find("/:") {
        let name_start = at + 2;
        let name_len = rest[name_start..].find(['.', '/']).unwrap_or(rest.len() - name_start);
        let name_end = name_start + name_len;
        let name = &rest[name_start..name_end];
        // As in `Resolver`, the last entry with the name wins.
        let value = vars.iter().rev().find(|(key, _)| key == name).map(|(_, value)| value.as_str());
        match value {
            Some(value) if !name.is_empty() && !value.is_empty() => {
                out.push_str(&rest[..=at]);
                out.push_str(value);
            }
            _ => out.push_str(&rest[..name_end]),
        }
        rest = &rest[name_end..];
    }
    out.push_str(rest);
    out.push_str(&url[path_end..]);
    out
}
