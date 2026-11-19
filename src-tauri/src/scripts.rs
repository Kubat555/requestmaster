//! The scripts of [`AppCore`]: what the scripts of a send left, in the form the window and the history get
//! it, and the consent to run the scripts of a collection.

use rqm_model::NodeKind;
use rqm_runner::{
    AppliedChange, ConsoleEntry, ConsoleLevel, ScriptErrorKind, ScriptFailure, ScriptPhase, ScriptSource,
    StoredScope, TestOutcome, VariableOp, SCRIPTS_ALLOWED_KEY,
};
use rqm_store::Store;

use crate::core::{message, AppCore};
use crate::dto::{
    ConsoleLevelDto, ConsoleLineDto, ScriptErrorDto, ScriptErrorKindDto, ScriptPhaseDto, ScriptSourceDto,
    TestResultDto, VariableChangeDto, VariableOpDto, VariableScope,
};

/// The most characters of a node name a [`ScriptSourceDto`] keeps.
const NODE_NAME_LIMIT: usize = 200;

impl AppCore {
    /// Whether the scripts of the collection of the node may run; `node_id` is a collection, a folder or
    /// a request. An unknown node is an error.
    pub fn scripts_allowed(&self, node_id: &str) -> Result<bool, String> {
        let store = self.store()?;
        let collection = rqm_runner::collection_of(&store, node_id).map_err(message)?;
        rqm_runner::scripts_allowed(&store, &collection).map_err(message)
    }

    /// Gives or takes back the consent to run the scripts of the collection of the node; `node_id` is
    /// a collection, a folder or a request. An unknown node is an error.
    pub fn set_scripts_allowed(&self, node_id: &str, allowed: bool) -> Result<(), String> {
        let mut store = self.store()?;
        let collection = rqm_runner::collection_of(&store, node_id).map_err(message)?;
        rqm_runner::set_scripts_allowed(&mut store, &collection, allowed).map_err(message)
    }
}

/// Scripts are allowed unless the user forbids them for a collection. A database written before the consent
/// existed has no list of allowed collections: it is created once, with every collection the database holds.
/// A database that has the list is left as it is.
pub(crate) fn allow_existing_collections(store: &mut Store) -> Result<(), String> {
    if store.state_get(SCRIPTS_ALLOWED_KEY).map_err(message)?.is_some() {
        return Ok(());
    }
    let rows = store.tree().map_err(message)?;
    let collections: Vec<String> =
        rows.into_iter().filter(|row| row.kind == NodeKind::Collection).map(|row| row.id).collect();
    store.state_set(SCRIPTS_ALLOWED_KEY, &serde_json::Value::from(collections).to_string()).map_err(message)
}

impl From<ScriptSource> for ScriptSourceDto {
    fn from(source: ScriptSource) -> Self {
        ScriptSourceDto {
            node_id: source.node_id,
            node_name: source.node_name.chars().take(NODE_NAME_LIMIT).collect(),
            node_kind: source.node_kind,
            phase: match source.phase {
                ScriptPhase::PreRequest => ScriptPhaseDto::PreRequest,
                ScriptPhase::Test => ScriptPhaseDto::Test,
            },
        }
    }
}

impl From<TestOutcome> for TestResultDto {
    fn from(test: TestOutcome) -> Self {
        TestResultDto {
            source: test.source.into(),
            name: test.name,
            passed: test.passed,
            skipped: test.skipped,
            error: test.error,
        }
    }
}

impl From<ScriptFailure> for ScriptErrorDto {
    fn from(failure: ScriptFailure) -> Self {
        ScriptErrorDto {
            source: failure.source.into(),
            kind: match failure.kind {
                ScriptErrorKind::Exception => ScriptErrorKindDto::Exception,
                ScriptErrorKind::Timeout => ScriptErrorKindDto::Timeout,
                ScriptErrorKind::Memory => ScriptErrorKindDto::Memory,
                ScriptErrorKind::Cancelled => ScriptErrorKindDto::Cancelled,
                ScriptErrorKind::Internal => ScriptErrorKindDto::Internal,
            },
            message: failure.message,
            line: failure.line,
            column: failure.column,
        }
    }
}

impl From<ConsoleEntry> for ConsoleLineDto {
    fn from(entry: ConsoleEntry) -> Self {
        ConsoleLineDto {
            source: entry.source.map(Into::into),
            level: match entry.level {
                ConsoleLevel::Log => ConsoleLevelDto::Log,
                ConsoleLevel::Info => ConsoleLevelDto::Info,
                ConsoleLevel::Warn => ConsoleLevelDto::Warn,
                ConsoleLevel::Error => ConsoleLevelDto::Error,
                ConsoleLevel::Debug => ConsoleLevelDto::Debug,
                ConsoleLevel::Network => ConsoleLevelDto::Network,
            },
            text: entry.text,
            at_ms: entry.at_ms,
        }
    }
}

impl From<AppliedChange> for VariableChangeDto {
    fn from(change: AppliedChange) -> Self {
        let (op, key, value) = match change.op {
            VariableOp::Set { key, value } => (VariableOpDto::Set, key, Some(value)),
            VariableOp::Unset { key } => (VariableOpDto::Unset, key, None),
            VariableOp::Clear => (VariableOpDto::Clear, String::new(), None),
        };
        let scope = match change.scope {
            StoredScope::Globals => VariableScope::Global,
            StoredScope::Collection => VariableScope::Collection,
            StoredScope::Environment => VariableScope::Environment,
        };
        VariableChangeDto { scope, target_id: change.target_id, op, key, value }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The name is repeated in every test of a send: it must not bloat the answer and the history entry.
    #[test]
    fn node_name_is_cut_at_200_characters() {
        let source = ScriptSource {
            node_id: "id".to_string(),
            node_name: "я".repeat(300),
            node_kind: NodeKind::Folder,
            phase: ScriptPhase::Test,
        };
        assert_eq!(ScriptSourceDto::from(source).node_name, "я".repeat(200));
    }
}
