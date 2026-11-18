//! The variables a script sees and changes: the four lists, the rules of `set / unset / clear`, their limits,
//! the log of changes of the stored scopes, and the `Resolver` behind `replaceIn`.

use crate::host::{HostLimit, Refusal};
use crate::text;
use rqm_vars::{Resolver, Scope, ScopeKind};

/// The longest value (UTF-8 bytes of `text`, and of `typed` separately) a script may set.
pub const MAX_VALUE_BYTES: usize = 1_048_576;
/// The most the scripts of a job may add to its variables: keys, `text` and `typed` of all four lists, over
/// what the job came with.
pub const MAX_VARIABLES_BYTES: usize = 16 * 1_048_576;
/// The most entries and bytes (keys and values) of `Report::changes`.
pub const MAX_CHANGES: usize = 10_000;
pub const MAX_CHANGES_BYTES: usize = 16 * 1_048_576;

/// How much of a key an error text quotes, in characters.
const KEY_IN_ERROR_CHARS: usize = 100;

/// One variable a script sees. `typed` — JSON of the value a script of this send has set (a number, an object…).
#[derive(Debug, Clone, PartialEq)]
pub struct Var {
    pub key: String,
    pub text: String,
    pub typed: Option<String>,
}

impl Var {
    pub fn new(key: impl Into<String>, text: impl Into<String>) -> Self {
        Var {
            key: key.into(),
            text: text.into(),
            typed: None,
        }
    }

    fn bytes(&self) -> usize {
        entry_bytes(&self.key, &self.text, self.typed.as_deref())
    }
}

fn entry_bytes(key: &str, text: &str, typed: Option<&str>) -> usize {
    key.len()
        .saturating_add(text.len())
        .saturating_add(typed.map_or(0, str::len))
}

/// The variables of a send: enabled ones only, in their stored order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Variables {
    pub globals: Vec<Var>,
    pub collection: Vec<Var>,
    pub environment: Vec<Var>,
    /// `None` — no environment is chosen: `pm.environment` works but nothing is stored.
    pub environment_name: Option<String>,
    pub local: Vec<Var>,
}

impl Variables {
    /// Global < Collection < Environment < Local, by `text`.
    pub fn resolver(&self) -> Resolver {
        Resolver::new(vec![
            scope(ScopeKind::Global, &self.globals),
            scope(ScopeKind::Collection, &self.collection),
            scope(ScopeKind::Environment, &self.environment),
            scope(ScopeKind::Local, &self.local),
        ])
    }

    fn bytes(&self) -> usize {
        [
            &self.globals,
            &self.collection,
            &self.environment,
            &self.local,
        ]
        .into_iter()
        .flatten()
        .fold(0, |sum, var| sum.saturating_add(var.bytes()))
    }
}

fn scope(kind: ScopeKind, list: &[Var]) -> Scope {
    Scope {
        kind,
        values: list
            .iter()
            .map(|var| (var.key.clone(), var.text.clone()))
            .collect(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredScope {
    Globals,
    Collection,
    Environment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableOp {
    Set { key: String, value: String },
    Unset { key: String },
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableChange {
    pub scope: StoredScope,
    pub op: VariableOp,
}

/// The list `set / unset / clear` change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Stored(StoredScope),
    /// `pm.variables`: lives for the send, never stored, never logged.
    Local,
}

impl Target {
    /// The name of the scope in `pm`.
    fn name(self) -> &'static str {
        match self {
            Target::Stored(StoredScope::Globals) => "globals",
            Target::Stored(StoredScope::Collection) => "collectionVariables",
            Target::Stored(StoredScope::Environment) => "environment",
            Target::Local => "variables",
        }
    }
}

/// What `get / has / toObject / replaceIn` read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    One(Target),
    /// `pm.variables`: the narrowest scope that has the key wins.
    Effective,
}

impl View {
    fn name(self) -> &'static str {
        match self {
            View::One(target) => target.name(),
            View::Effective => Target::Local.name(),
        }
    }

    /// The error of a key that is too long to be read from the script at all.
    pub(crate) fn key_too_long(self, method: &str) -> String {
        key_too_long(self.name(), method)
    }
}

fn key_too_long(scope: &str, method: &str) -> String {
    format!("pm.{scope}.{method}: the key is larger than 1 MB")
}

/// The errors of `set` a script may catch. `None` — the sizes are fine. Takes sizes, not texts: the border
/// asks before it copies a value out of JavaScript.
pub(crate) fn set_size_error(
    target: Target,
    key: Option<&str>,
    key_bytes: usize,
    text_bytes: usize,
    typed_bytes: usize,
) -> Option<String> {
    if key_bytes > MAX_VALUE_BYTES {
        return Some(key_too_long(target.name(), "set"));
    }
    if text_bytes > MAX_VALUE_BYTES || typed_bytes > MAX_VALUE_BYTES {
        let key = text::clip(key.unwrap_or_default(), KEY_IN_ERROR_CHARS);
        return Some(format!(
            "pm.{}.set: the value of '{key}' is larger than 1 MB",
            target.name()
        ));
    }
    None
}

/// The variables of a job while its scripts run, and the log of what the scripts changed in the stored scopes.
#[derive(Debug)]
pub(crate) struct Book {
    pub(crate) variables: Variables,
    pub(crate) changes: Vec<VariableChange>,
    /// Bytes of `variables` when the job came: a big environment of the user is not the limit.
    base_bytes: usize,
    /// Bytes of `variables` now.
    bytes: usize,
    /// Bytes of the keys and values in `changes`.
    change_bytes: usize,
}

impl Book {
    pub(crate) fn new(variables: Variables) -> Self {
        let bytes = variables.bytes();
        Book {
            variables,
            changes: Vec::new(),
            base_bytes: bytes,
            bytes,
            change_bytes: 0,
        }
    }

    fn list(&self, target: Target) -> &Vec<Var> {
        match target {
            Target::Stored(StoredScope::Globals) => &self.variables.globals,
            Target::Stored(StoredScope::Collection) => &self.variables.collection,
            Target::Stored(StoredScope::Environment) => &self.variables.environment,
            Target::Local => &self.variables.local,
        }
    }

    fn list_mut(&mut self, target: Target) -> &mut Vec<Var> {
        match target {
            Target::Stored(StoredScope::Globals) => &mut self.variables.globals,
            Target::Stored(StoredScope::Collection) => &mut self.variables.collection,
            Target::Stored(StoredScope::Environment) => &mut self.variables.environment,
            Target::Local => &mut self.variables.local,
        }
    }

    /// The scope the changes of `target` are logged under. `None` — they are not logged: local variables, and
    /// the environment when none is chosen (there is nowhere to store them).
    pub(crate) fn logged_scope(&self, target: Target) -> Option<StoredScope> {
        match target {
            Target::Local => None,
            Target::Stored(StoredScope::Environment)
                if self.variables.environment_name.is_none() =>
            {
                None
            }
            Target::Stored(scope) => Some(scope),
        }
    }

    /// The last entry with the key.
    pub(crate) fn find(&self, view: View, key: &str) -> Option<&Var> {
        let last = |target: Target| self.list(target).iter().rev().find(|var| var.key == key);
        match view {
            View::One(target) => last(target),
            View::Effective => last(Target::Local)
                .or_else(|| last(Target::Stored(StoredScope::Environment)))
                .or_else(|| last(Target::Stored(StoredScope::Collection)))
                .or_else(|| last(Target::Stored(StoredScope::Globals))),
        }
    }

    /// The entries of the view from the widest scope to the narrowest, in their stored order.
    pub(crate) fn entries(&self, view: View) -> Vec<&Var> {
        match view {
            View::One(target) => self.list(target).iter().collect(),
            View::Effective => self
                .variables
                .globals
                .iter()
                .chain(&self.variables.collection)
                .chain(&self.variables.environment)
                .chain(&self.variables.local)
                .collect(),
        }
    }

    /// `{{name}}` in `input` replaced by the variables of the view.
    pub(crate) fn replace(&self, view: View, input: &str) -> String {
        let resolver = match view {
            View::Effective => self.variables.resolver(),
            View::One(target) => Resolver::new(vec![scope(ScopeKind::Local, self.list(target))]),
        };
        resolver.substitute(input).text
    }

    /// Changes the last entry with the key, or adds one at the end.
    pub(crate) fn set(
        &mut self,
        target: Target,
        key: String,
        text: String,
        typed: Option<String>,
    ) -> Result<(), Refusal> {
        let typed_bytes = typed.as_ref().map_or(0, String::len);
        if let Some(error) = set_size_error(target, Some(&key), key.len(), text.len(), typed_bytes)
        {
            return Err(Refusal::Error(error));
        }
        let position = self.list(target).iter().rposition(|var| var.key == key);
        let old_bytes = position
            .and_then(|position| self.list(target).get(position))
            .map_or(0, Var::bytes);
        let bytes = self
            .bytes
            .saturating_sub(old_bytes)
            .saturating_add(entry_bytes(&key, &text, typed.as_deref()));
        if bytes > self.base_bytes.saturating_add(MAX_VARIABLES_BYTES) {
            return Err(Refusal::Limit(HostLimit::VariableBytes));
        }
        let logged = self.logged_scope(target);
        if logged.is_some() {
            self.check_log(key.len().saturating_add(text.len()))?;
        }

        if let Some(scope) = logged {
            self.log(
                scope,
                VariableOp::Set {
                    key: key.clone(),
                    value: text.clone(),
                },
                key.len().saturating_add(text.len()),
            );
        }
        self.bytes = bytes;
        let list = self.list_mut(target);
        match position.and_then(|position| list.get_mut(position)) {
            Some(var) => {
                var.text = text;
                var.typed = typed;
            }
            None => list.push(Var { key, text, typed }),
        }
        Ok(())
    }

    /// Removes every entry with the key. A key that is not there changes nothing and is not logged.
    pub(crate) fn unset(&mut self, target: Target, key: &str) -> Result<(), Refusal> {
        if !self.list(target).iter().any(|var| var.key == key) {
            return Ok(());
        }
        let removed = self
            .list(target)
            .iter()
            .filter(|var| var.key == key)
            .fold(0usize, |sum, var| sum.saturating_add(var.bytes()));
        if let Some(scope) = self.logged_scope(target) {
            self.check_log(key.len())?;
            self.log(
                scope,
                VariableOp::Unset {
                    key: key.to_string(),
                },
                key.len(),
            );
        }
        self.list_mut(target).retain(|var| var.key != key);
        self.bytes = self.bytes.saturating_sub(removed);
        Ok(())
    }

    /// Removes every entry of the scope. An empty scope changes nothing and is not logged.
    pub(crate) fn clear(&mut self, target: Target) -> Result<(), Refusal> {
        if self.list(target).is_empty() {
            return Ok(());
        }
        if let Some(scope) = self.logged_scope(target) {
            self.check_log(0)?;
            self.log(scope, VariableOp::Clear, 0);
        }
        let removed = self
            .list(target)
            .iter()
            .fold(0usize, |sum, var| sum.saturating_add(var.bytes()));
        self.list_mut(target).clear();
        self.bytes = self.bytes.saturating_sub(removed);
        Ok(())
    }

    /// Whether the log takes one more entry of `bytes` bytes.
    fn check_log(&self, bytes: usize) -> Result<(), Refusal> {
        if self.changes.len() >= MAX_CHANGES {
            return Err(Refusal::Limit(HostLimit::ChangeCount));
        }
        if self.change_bytes.saturating_add(bytes) > MAX_CHANGES_BYTES {
            return Err(Refusal::Limit(HostLimit::ChangeBytes));
        }
        Ok(())
    }

    fn log(&mut self, scope: StoredScope, op: VariableOp, bytes: usize) {
        self.changes.push(VariableChange { scope, op });
        self.change_bytes = self.change_bytes.saturating_add(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLOBALS: Target = Target::Stored(StoredScope::Globals);

    fn set(book: &mut Book, target: Target, key: &str, text: String) -> Result<(), Refusal> {
        book.set(target, key.to_string(), text, None)
    }

    #[test]
    fn variable_primitives_enforce_their_limits() {
        // A value that is too long is an error the script may catch; nothing changes.
        let mut book = Book::new(Variables {
            globals: vec![Var::new("k", "old")],
            ..Variables::default()
        });
        let refused = set(&mut book, GLOBALS, "k", "x".repeat(MAX_VALUE_BYTES + 1));
        assert_eq!(
            refused,
            Err(Refusal::Error(
                "pm.globals.set: the value of 'k' is larger than 1 MB".to_string()
            ))
        );
        let typed = Some("x".repeat(MAX_VALUE_BYTES + 1));
        assert!(matches!(
            book.set(GLOBALS, "k".to_string(), String::new(), typed),
            Err(Refusal::Error(_))
        ));
        assert_eq!(book.variables.globals, vec![Var::new("k", "old")]);
        assert_eq!(book.changes, vec![]);
        assert_eq!(
            set(&mut book, GLOBALS, "k", "x".repeat(MAX_VALUE_BYTES)),
            Ok(())
        );

        // The log takes `MAX_CHANGES` entries and not one more.
        let mut book = Book::new(Variables::default());
        for _ in 0..MAX_CHANGES {
            assert_eq!(set(&mut book, GLOBALS, "i", "1".to_string()), Ok(()));
        }
        assert_eq!(
            set(&mut book, GLOBALS, "i", "2".to_string()),
            Err(Refusal::Limit(HostLimit::ChangeCount))
        );
        assert_eq!(book.changes.len(), MAX_CHANGES);
        assert_eq!(book.variables.globals, vec![Var::new("i", "1")]);

        // The bytes of the log: one key, so the list does not grow.
        let mut book = Book::new(Variables::default());
        let half = MAX_VALUE_BYTES / 2;
        let mut refused = None;
        for _ in 0..40 {
            if let Err(refusal) = set(&mut book, GLOBALS, "k", "x".repeat(half)) {
                refused = Some(refusal);
                break;
            }
        }
        assert_eq!(refused, Some(Refusal::Limit(HostLimit::ChangeBytes)));

        // The variables may grow by `MAX_VARIABLES_BYTES` over what the job came with — local ones too.
        let mut book = Book::new(Variables {
            environment: vec![Var::new("big", "x".repeat(20 << 20))],
            ..Variables::default()
        });
        let mut refused = None;
        let mut stored = 0;
        for index in 0..40 {
            match set(
                &mut book,
                Target::Local,
                &format!("k{index}"),
                "x".repeat(MAX_VALUE_BYTES),
            ) {
                Ok(()) => stored += 1,
                Err(refusal) => {
                    refused = Some(refusal);
                    break;
                }
            }
        }
        assert_eq!(refused, Some(Refusal::Limit(HostLimit::VariableBytes)));
        assert_eq!(stored, 15);
        assert_eq!(book.variables.local.len(), 15);
        assert_eq!(book.changes, vec![]);
    }
}
