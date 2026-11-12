//! Postman import for RequestMaster: collections (schema v2.1), environments and globals
//! are read from `serde_json::Value` into the `rqm-model` types.
//!
//! Reading is tolerant: a value of an unexpected type counts as a missing field, unknown keys are
//! kept in `postman_extra` (rule P1 of the plan), and nothing unknown fails the import.

mod detect;
mod diff;
mod import;
mod report;

pub use detect::{detect, parse_text, Detected, ImportError};
pub use diff::semantic_diff;
pub use import::{import_collection, import_environment, ImportedCollection};
pub use report::{ImportReport, ReportItem, ReportKind};
