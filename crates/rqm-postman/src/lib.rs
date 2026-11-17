//! Postman import and export for RequestMaster: collections (schema v2.1), environments and globals
//! are read from `serde_json::Value` into the `rqm-model` types and written back.
//!
//! Reading is tolerant: a value of an unexpected type counts as a missing field, unknown keys are
//! kept in `postman_extra` (rule P1 of the plan), and nothing unknown fails the import.
//! Writing starts from `postman_extra` and replaces only what was changed (rules P2-P4).

mod description;
mod detect;
mod diff;
mod export;
mod globals_merge;
mod import;
mod report;

pub use description::{description_of, set_description};
pub use detect::{detect, parse_text, Detected, ImportError};
pub use diff::semantic_diff;
pub use export::{export_collection, export_environment, ExportError};
pub use globals_merge::{globals_conflicts, merge_globals, MergePolicy};
pub use import::{import_collection, import_environment, ImportedCollection};
pub use report::{ImportReport, ReportItem, ReportKind};
