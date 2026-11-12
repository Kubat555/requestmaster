//! Import report: what was read from the file but will not work as in Postman.

use rqm_model::NodeId;

/// Everything the import wants the user to know about; filled by `import_collection`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportReport {
    /// Records in the order of the nodes in the file.
    pub items: Vec<ReportItem>,
}

/// One record of the import report.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportItem {
    /// What the record is about.
    pub kind: ReportKind,
    /// The imported node the record belongs to.
    pub node_id: NodeId,
    /// Names from the collection down to the node, joined with ` / `.
    pub node_path: String,
    /// Depends on `kind`: the Postman auth type, the setting key, the script events
    /// or the form field name (`binary` for a binary body). Never a file path.
    pub detail: String,
}

/// Kind of an import report record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// A report record before it is attached to a node: its kind and `detail`.
pub(crate) type Note = (ReportKind, String);
