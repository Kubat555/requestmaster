//! The scripts of [`AppCore`]: what the scripts of a send left, in the form the window and the history get
//! it, the consent to run the scripts of a collection, and what the import report says about scripts.

use std::collections::BTreeSet;

use rqm_model::{Node, NodeContent, NodeKind, Scripts};
use rqm_runner::{
    AppliedChange, ConsoleEntry, ConsoleLevel, ScriptErrorKind, ScriptFailure, ScriptPhase, ScriptSource,
    StoredScope, TestOutcome, VariableOp, SCRIPTS_ALLOWED_KEY,
};
use rqm_store::Store;

use crate::core::{message, AppCore};
use crate::dto::{
    ConsoleLevelDto, ConsoleLineDto, ReportItem, ReportKind, ScriptErrorDto, ScriptErrorKindDto, ScriptPhaseDto,
    ScriptSourceDto, TestResultDto, VariableChangeDto, VariableOpDto, VariableScope,
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

/// The report of an imported collection as the window gets it. Right after the `HasScripts` record of a node
/// whose scripts use sandbox API the engine does not support comes a record that names that API: sorted,
/// without repeats, joined with `, `.
pub(crate) fn import_report(nodes: &[Node], items: Vec<rqm_postman::ReportItem>) -> Vec<ReportItem> {
    let mut report = Vec::with_capacity(items.len());
    for item in items {
        let unsupported = match item.kind {
            rqm_postman::ReportKind::HasScripts => {
                nodes.iter().find(|node| node.id == item.node_id).map(|node| unsupported_in(scripts_of(&node.content)))
            }
            _ => None,
        };
        let (node_id, node_path) = (item.node_id.clone(), item.node_path.clone());
        report.push(ReportItem::from(item));
        if let Some(detail) = unsupported.filter(|names| !names.is_empty()) {
            report.push(ReportItem { kind: ReportKind::UnsupportedScriptApi, node_id, node_path, detail });
        }
    }
    report
}

/// The scripts of a node of any kind.
fn scripts_of(content: &NodeContent) -> &Scripts {
    match content {
        NodeContent::Collection(collection) => &collection.scripts,
        NodeContent::Folder(folder) => &folder.scripts,
        NodeContent::Request(request) => &request.scripts,
    }
}

/// The names of the unsupported sandbox API in both scripts of a node: sorted, without repeats, joined with
/// `, `; empty when the scripts use none.
fn unsupported_in(scripts: &Scripts) -> String {
    let names: BTreeSet<String> = [&scripts.pre_request, &scripts.test]
        .into_iter()
        .flat_map(|script| rqm_runner::unsupported_api(script))
        .collect();
    names.into_iter().collect::<Vec<_>>().join(", ")
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
