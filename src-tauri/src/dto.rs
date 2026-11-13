//! Data the window exchanges with the commands; JSON keys are `snake_case` (the command contract of the plan).

use rqm_model::NodeKind;
use serde::{Deserialize, Serialize};

/// A node as the tree shows it: everything except the content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeRow {
    /// Id of the node.
    pub id: String,
    /// `None` for a collection.
    pub parent_id: Option<String>,
    /// Kind of the node.
    pub kind: NodeKind,
    /// Name shown in the tree.
    pub name: String,
    /// Position among the siblings, 0..n.
    pub sort_order: i64,
    /// HTTP method of a request; `None` for folders and collections.
    pub method: Option<String>,
}

impl From<rqm_store::TreeRow> for TreeRow {
    fn from(row: rqm_store::TreeRow) -> Self {
        TreeRow {
            id: row.id,
            parent_id: row.parent_id,
            kind: row.kind,
            name: row.name,
            sort_order: row.sort_order,
            method: row.method,
        }
    }
}

/// A node with its content, as an editor tab opens it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeDto {
    /// Id of the node.
    pub id: String,
    /// `None` for a collection.
    pub parent_id: Option<String>,
    /// Kind of the node; decides the shape of `content`.
    pub kind: NodeKind,
    /// Name shown in the tree.
    pub name: String,
    /// JSON of `NodeContent::to_json` without the top-level `postman_extra` key.
    pub content: serde_json::Value,
}

/// A file the user chose for import: the window reads it and passes the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportFile {
    /// File name without the folder; returned in the result so the window can tell the files apart.
    pub name: String,
    /// Contents of the file as text.
    pub text: String,
}

/// What a file turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    /// A Postman collection; it was added to the tree.
    Collection,
    /// A Postman environment; it was saved.
    Environment,
    /// A Postman globals file; merged unless `conflicts` is not empty.
    Globals,
    /// The file was not imported; `error` says why.
    Error,
}

/// Outcome of importing one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportResult {
    /// `ImportFile::name` of the file this result is about.
    pub file_name: String,
    /// What the file turned out to be.
    pub kind: ImportKind,
    /// Name of the imported collection, environment or globals set; `None` for an error.
    pub name: Option<String>,
    /// Id of the new collection node; `None` for anything but a collection.
    pub root_id: Option<String>,
    /// What was read from a collection but will not work as in Postman; empty for other kinds.
    pub report: Vec<ReportItem>,
    /// Globals only: names whose stored value differs from the file's. When not empty nothing was applied
    /// and the window asks the user, then calls `import_globals` with the chosen policy.
    pub conflicts: Vec<String>,
    /// Why the file was not imported; `Some` only for `ImportKind::Error`.
    pub error: Option<String>,
}

/// Kind of an import report record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    /// The node has a Postman authorization type the application does not apply.
    UnsupportedAuth,
    /// A setting is kept for export but has no effect in the application.
    SettingNotApplied,
    /// The node has a non-empty pre-request or test script.
    HasScripts,
    /// The body refers to a file on the computer the collection was exported from.
    FileReference,
}

/// One record of the import report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportItem {
    /// What the record is about.
    pub kind: ReportKind,
    /// The imported node the record belongs to.
    pub node_id: String,
    /// Names from the collection down to the node, joined with ` / `.
    pub node_path: String,
    /// Depends on `kind`: the Postman auth type, the setting key, the script events or the form field name.
    pub detail: String,
}

impl From<rqm_postman::ReportItem> for ReportItem {
    fn from(item: rqm_postman::ReportItem) -> Self {
        let kind = match item.kind {
            rqm_postman::ReportKind::UnsupportedAuth => ReportKind::UnsupportedAuth,
            rqm_postman::ReportKind::SettingNotApplied => ReportKind::SettingNotApplied,
            rqm_postman::ReportKind::HasScripts => ReportKind::HasScripts,
            rqm_postman::ReportKind::FileReference => ReportKind::FileReference,
        };
        ReportItem { kind, node_id: item.node_id, node_path: item.node_path, detail: item.detail }
    }
}

/// An environment in the switcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvSummary {
    /// Id of the environment.
    pub id: String,
    /// Name shown in the switcher.
    pub name: String,
}
