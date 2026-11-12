//! RequestMaster domain model: the tree of collections, folders and requests, and environments.
//!
//! The crate holds data only. Content structures are written to the store as JSON:
//! default values and empty `postman_extra` are skipped on write and restored on read.

mod environment;
mod node;
mod request;

pub use environment::{EnvKind, Environment, GLOBALS_ID};
pub use node::{CollectionContent, FolderContent, Node, NodeContent, NodeKind};
pub use request::{
    ApiKeyPlacement, Auth, Body, FileRef, FormPart, FormValue, Param, RawLanguage, RequestContent,
    RequestSettings, Scripts, Variable,
};

/// Identifier of a tree node or an environment: UUID v4, lower-case hyphenated.
pub type NodeId = String;

/// Original Postman JSON object of a node or a list element (rules P1 and P3 of the plan).
/// Empty for anything created in the application.
pub type Extra = serde_json::Map<String, serde_json::Value>;

/// Generates a new random identifier (UUID v4, lower-case hyphenated).
pub fn new_id() -> NodeId {
    uuid::Uuid::new_v4().to_string()
}

/// `skip_serializing_if` helper: a value equal to its type's default is not written.
pub(crate) fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// Serializes a content structure to a JSON string.
pub(crate) fn to_json_string<T: serde::Serialize>(value: &T) -> String {
    // Cannot fail: the model has no maps with non-string keys and no fallible `Serialize` impls.
    serde_json::to_string(value).expect("model types always serialize to JSON")
}
