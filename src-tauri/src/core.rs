//! [`AppCore`]: everything the window does, without any Tauri type, so that it is covered by plain tests.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use rqm_model::{
    Body, CollectionContent, EnvKind, Environment, Extra, FolderContent, NodeContent, NodeKind, RequestContent,
    RequestSettings, Scripts, GLOBALS_ID,
};
use rqm_postman::{Detected, ImportError, MergePolicy};
use rqm_store::{Store, StoreError};

use crate::dto::{EnvSummary, EnvironmentDto, ImportFile, ImportKind, ImportResult, NodeDto, TreeRow, VariableDto};
use crate::send::SendState;

/// Key of `app_state` holding the id of the active environment as JSON: a string, or `null` when none is chosen.
pub const ACTIVE_ENVIRONMENT_KEY: &str = "active_environment";

/// How many history entries are kept when [`CoreConfig::history_keep`] does not say.
const DEFAULT_HISTORY_KEEP: usize = 500;

/// Where the core keeps its data.
pub struct CoreConfig {
    /// Database file; `None` = a database in memory (tests).
    pub db_path: Option<PathBuf>,
    /// Folder for the files of response bodies. It belongs to the core entirely: whatever a previous run
    /// left there is deleted when the core is created.
    pub bodies_dir: PathBuf,
    /// How many history entries are kept, the newest ones; `None` = 500.
    pub history_keep: Option<usize>,
}

/// Everything the window needs, without any Tauri type. One instance per application.
pub struct AppCore {
    /// The database. Locked for the time of one operation.
    store: Mutex<Store>,
    /// The engine, the cookies, the sends in flight and the stored response bodies (see `send.rs`).
    pub(crate) send: SendState,
    /// How many history entries are kept (see `history.rs`).
    pub(crate) history_keep: usize,
}

impl AppCore {
    /// Opens the database, empties the folder of response bodies and creates the HTTP engine with the
    /// cookies kept in the database.
    pub fn new(config: CoreConfig) -> Result<Self, String> {
        let store = match &config.db_path {
            Some(path) => Store::open(path),
            None => Store::open_in_memory(),
        }
        .map_err(message)?;
        match std::fs::remove_dir_all(&config.bodies_dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot clear the response bodies folder: {error}")),
        }
        std::fs::create_dir_all(&config.bodies_dir)
            .map_err(|error| format!("cannot create the response bodies folder: {error}"))?;
        let send = SendState::new(&store, config.bodies_dir)?;
        let history_keep = config.history_keep.unwrap_or(DEFAULT_HISTORY_KEEP);
        Ok(AppCore { store: Mutex::new(store), send, history_keep })
    }

    /// All nodes without content, ordered by (parent, position).
    pub fn tree(&self) -> Result<Vec<TreeRow>, String> {
        let rows = self.store()?.tree().map_err(message)?;
        Ok(rows.into_iter().map(TreeRow::from).collect())
    }

    /// The node with its content; the top-level `postman_extra` is cut out (see [`NodeDto::content`]).
    pub fn get_node(&self, id: &str) -> Result<NodeDto, String> {
        let mut node = self.store()?.node(id).map_err(message)?;
        top_extra(&mut node.content).clear();
        let content = serde_json::from_str(&node.content.to_json()).map_err(message)?;
        Ok(NodeDto { id: node.id, parent_id: node.parent_id, kind: node.kind, name: node.name, content })
    }

    /// Replaces the content of the node with `content` (the shape [`AppCore::get_node`] returns). The kind is
    /// taken from the store; the top-level `postman_extra` of the stored node is kept. Content that does not
    /// read as this kind is an error and changes nothing.
    pub fn save_node_content(&self, id: &str, content: serde_json::Value) -> Result<(), String> {
        let mut store = self.store()?;
        let mut stored = store.node(id).map_err(message)?;
        let mut updated = NodeContent::from_json(stored.kind, &content.to_string())
            .map_err(|error| format!("invalid content: {error}"))?;
        *top_extra(&mut updated) = std::mem::take(top_extra(&mut stored.content));
        store.update_content(id, &updated).map_err(message)
    }

    /// Creates a node as the last child of `parent_id` (`None` = as the last collection) and returns its id.
    /// The name is trimmed and must not be empty. A collection and a folder start empty; a request starts as
    /// `GET` with an empty address, no body and inherited authorization. A place the tree rules forbid is
    /// the store's error.
    pub fn create_node(&self, parent_id: Option<&str>, kind: NodeKind, name: &str) -> Result<String, String> {
        let name = node_name(name)?;
        self.store()?.create_node(parent_id, kind, name, &blank_content(kind)).map_err(|error| match error {
            // Nothing is moved here, so the window gets the broken rule itself: "a request cannot have children".
            StoreError::InvalidMove(rule) => rule,
            other => message(other),
        })
    }

    /// Changes the name of the node; the name is trimmed and must not be empty.
    pub fn rename_node(&self, id: &str, name: &str) -> Result<(), String> {
        let name = node_name(name)?;
        self.store()?.rename_node(id, name).map_err(message)
    }

    /// Copies the node with its descendants right after the original; returns the id of the copy.
    pub fn duplicate_node(&self, id: &str) -> Result<String, String> {
        self.store()?.duplicate_node(id).map_err(message)
    }

    /// Moves the node under `parent_id` (`None` = among the collections) to position `index`, counted among
    /// the children of that parent without the moved node; an index past the end means the last place.
    /// A place the tree rules forbid is an error that names the broken rule, and nothing changes.
    pub fn move_node(&self, id: &str, parent_id: Option<&str>, index: usize) -> Result<(), String> {
        self.store()?.move_node(id, parent_id, index).map_err(|error| match error {
            // As in `create_node`: the window gets the rule itself, "a request cannot have children".
            StoreError::InvalidMove(rule) => rule,
            other => message(other),
        })
    }

    /// Deletes the node with its descendants.
    pub fn delete_node(&self, id: &str) -> Result<(), String> {
        self.store()?.delete_node(id).map_err(message)
    }

    /// Imports Postman files. Every file is independent: one that cannot be imported gives a result of kind
    /// `error` and does not stop the others.
    pub fn import_files(&self, files: Vec<ImportFile>) -> Vec<ImportResult> {
        files
            .into_iter()
            .map(|file| {
                let mut result = self.import_text(&file.text).unwrap_or_else(|error| ImportResult {
                    error: Some(error),
                    ..blank_result(ImportKind::Error)
                });
                result.file_name = file.name;
                result
            })
            .collect()
    }

    /// Merges a globals file into the stored globals; `policy` settles the variables whose value differs:
    /// `take_incoming` or `keep_current`.
    pub fn import_globals(&self, text: &str, policy: &str) -> Result<(), String> {
        let policy = match policy {
            "take_incoming" => MergePolicy::TakeIncoming,
            "keep_current" => MergePolicy::KeepCurrent,
            other => return Err(format!("unknown merge policy `{other}`")),
        };
        let json = rqm_postman::parse_text(text).map_err(import_error)?;
        if rqm_postman::detect(&json).map_err(import_error)? != Detected::Globals {
            return Err("not a globals file".to_string());
        }
        let incoming = rqm_postman::import_environment(&json).map_err(import_error)?;
        let mut store = self.store()?;
        let current = store.globals().map_err(message)?;
        save_merged_globals(&mut store, current, incoming, policy)
    }

    /// Environments of the switcher, by name; globals are not included.
    pub fn environments(&self) -> Result<Vec<EnvSummary>, String> {
        let environments = self.store()?.environments().map_err(message)?;
        Ok(environments.into_iter().map(|env| EnvSummary { id: env.id, name: env.name }).collect())
    }

    /// Id of the active environment; `None` when none is chosen or the chosen one no longer exists.
    pub fn active_environment(&self) -> Result<Option<String>, String> {
        active_environment_in(&*self.store()?)
    }

    /// Chooses the active environment; `None` clears the choice. An id that is not an environment is an error.
    pub fn set_active_environment(&self, id: Option<&str>) -> Result<(), String> {
        let mut store = self.store()?;
        if let Some(id) = id {
            if !environment_exists(&store, id)? {
                return Err(format!("environment not found: {id}"));
            }
        }
        let value = serde_json::to_string(&id).map_err(message)?;
        store.state_set(ACTIVE_ENVIRONMENT_KEY, &value).map_err(message)
    }

    /// The environment with its variables; the id `globals` gives the globals set, empty while nothing is
    /// stored. Any other id that is not an environment is an error.
    pub fn get_environment(&self, id: &str) -> Result<EnvironmentDto, String> {
        let environment = stored_environment(&*self.store()?, id)?;
        Ok(EnvironmentDto {
            id: environment.id,
            name: environment.name,
            variables: environment.variables.into_iter().map(VariableDto::from).collect(),
        })
    }

    /// Replaces the name and the variables of the environment. The name is trimmed and must not be empty;
    /// for the id `globals` it is ignored: the globals set keeps its name. The Postman source object of the
    /// environment stays as stored. An id that is not an environment is an error: nothing is created.
    pub fn save_environment(&self, id: &str, name: &str, variables: Vec<VariableDto>) -> Result<(), String> {
        let mut store = self.store()?;
        let mut environment = stored_environment(&store, id)?;
        if id != GLOBALS_ID {
            environment.name = node_name(name)?.to_string();
        }
        environment.variables = variables.into_iter().map(Into::into).collect();
        store.save_environment(&environment).map_err(message)
    }

    /// Creates an environment without variables and returns its id; the name is trimmed and must not be empty.
    pub fn create_environment(&self, name: &str) -> Result<String, String> {
        let environment = Environment {
            id: rqm_model::new_id(),
            kind: EnvKind::Environment,
            name: node_name(name)?.to_string(),
            variables: Vec::new(),
            postman_extra: Extra::new(),
        };
        self.store()?.save_environment(&environment).map_err(message)?;
        Ok(environment.id)
    }

    /// Deletes the environment; when it was the active one, the choice is cleared. The globals set cannot
    /// be deleted.
    pub fn delete_environment(&self, id: &str) -> Result<(), String> {
        if id == GLOBALS_ID {
            return Err("globals cannot be deleted".to_string());
        }
        let mut store = self.store()?;
        let was_active = active_environment_in(&store)?.as_deref() == Some(id);
        store.delete_environment(id).map_err(message)?;
        if was_active {
            store.state_set(ACTIVE_ENVIRONMENT_KEY, "null").map_err(message)?;
        }
        Ok(())
    }

    /// Re-indents the JSON text of a request body by the rules of [`rqm_runner::format_json`]: only the
    /// whitespace between tokens changes. Text that is not valid JSON is an error.
    pub fn format_json(&self, text: &str) -> Result<String, String> {
        rqm_runner::format_json(text).ok_or_else(|| "not valid JSON".to_string())
    }

    /// The store, locked until the guard is dropped.
    pub(crate) fn store(&self) -> Result<MutexGuard<'_, Store>, String> {
        self.store.lock().map_err(|_| "storage is unavailable".to_string())
    }

    /// Imports the text of one file; `file_name` of the result is left empty for the caller.
    /// The text is parsed before the store is locked.
    fn import_text(&self, text: &str) -> Result<ImportResult, String> {
        let json = rqm_postman::parse_text(text).map_err(import_error)?;
        match rqm_postman::detect(&json).map_err(import_error)? {
            Detected::Collection => {
                let imported = rqm_postman::import_collection(&json).map_err(import_error)?;
                // `import_collection` always returns the collection node first.
                let root = imported.nodes.first().ok_or_else(|| import_error(ImportError::UnknownFormat))?;
                let (name, root_id) = (root.name.clone(), root.id.clone());
                self.store()?.insert_nodes(&imported.nodes).map_err(message)?;
                Ok(ImportResult {
                    name: Some(name),
                    root_id: Some(root_id),
                    report: imported.report.items.into_iter().map(Into::into).collect(),
                    ..blank_result(ImportKind::Collection)
                })
            }
            Detected::Environment => {
                let environment = rqm_postman::import_environment(&json).map_err(import_error)?;
                self.store()?.save_environment(&environment).map_err(message)?;
                Ok(ImportResult { name: Some(environment.name), ..blank_result(ImportKind::Environment) })
            }
            Detected::Globals => {
                let incoming = rqm_postman::import_environment(&json).map_err(import_error)?;
                let name = incoming.name.clone();
                let mut store = self.store()?;
                let current = store.globals().map_err(message)?;
                let conflicts = rqm_postman::globals_conflicts(&current.variables, &incoming.variables);
                if conflicts.is_empty() {
                    save_merged_globals(&mut store, current, incoming, MergePolicy::TakeIncoming)?;
                }
                Ok(ImportResult { name: Some(name), conflicts, ..blank_result(ImportKind::Globals) })
            }
        }
    }
}

/// Id of the active environment of `store`; `None` when none is chosen, the chosen one no longer exists or
/// the stored value cannot be read (a broken value must not stop every send).
pub(crate) fn active_environment_in(store: &Store) -> Result<Option<String>, String> {
    let Some(value) = store.state_get(ACTIVE_ENVIRONMENT_KEY).map_err(message)? else {
        return Ok(None);
    };
    let Ok(Some(id)) = serde_json::from_str::<Option<String>>(&value) else {
        return Ok(None);
    };
    Ok(environment_exists(store, &id)?.then_some(id))
}

/// Whether `store` has an environment with this id; the globals set is not one.
fn environment_exists(store: &Store, id: &str) -> Result<bool, String> {
    Ok(store.environments().map_err(message)?.iter().any(|environment| environment.id == id))
}

/// The environment `id` of `store`, or the globals set for the id `globals` (empty while nothing is stored).
/// Any other id is an error: the store itself would create an environment under an unknown id on saving.
pub(crate) fn stored_environment(store: &Store, id: &str) -> Result<Environment, String> {
    if id == GLOBALS_ID {
        return store.globals().map_err(message);
    }
    let environments = store.environments().map_err(message)?;
    environments.into_iter().find(|environment| environment.id == id).ok_or_else(|| format!("environment not found: {id}"))
}

/// The name of a node or an environment as it is stored: trimmed; an empty one is an error.
fn node_name(name: &str) -> Result<&str, String> {
    match name.trim() {
        "" => Err("name is empty".to_string()),
        trimmed => Ok(trimmed),
    }
}

/// Content of a node created in the application: an empty collection or folder, or a `GET` request with an
/// empty address, no body and inherited authorization.
fn blank_content(kind: NodeKind) -> NodeContent {
    match kind {
        NodeKind::Collection => NodeContent::Collection(CollectionContent {
            auth: None,
            variables: Vec::new(),
            scripts: Scripts::default(),
            postman_extra: Extra::new(),
        }),
        NodeKind::Folder => {
            NodeContent::Folder(FolderContent { auth: None, scripts: Scripts::default(), postman_extra: Extra::new() })
        }
        NodeKind::Request => NodeContent::Request(RequestContent {
            method: "GET".to_string(),
            url: String::new(),
            query: Vec::new(),
            path_variables: Vec::new(),
            headers: Vec::new(),
            body: Body::None,
            auth: None,
            scripts: Scripts::default(),
            settings: RequestSettings::default(),
            disabled_auto_headers: Vec::new(),
            postman_extra: Extra::new(),
        }),
    }
}

/// Message of an error for the window.
pub(crate) fn message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Why a file was not imported, in the words the window shows.
fn import_error(error: ImportError) -> String {
    match error {
        ImportError::NotJson(_) => "not a JSON file",
        ImportError::UnknownFormat => "unknown format",
        // The only schema `detect` reports as unsupported is v2.0.0.
        ImportError::UnsupportedSchema(_) => "Collection v2.0 is not supported yet",
    }
    .to_string()
}

/// A result of this kind with every other field empty.
fn blank_result(kind: ImportKind) -> ImportResult {
    ImportResult {
        file_name: String::new(),
        kind,
        name: None,
        root_id: None,
        report: Vec::new(),
        conflicts: Vec::new(),
        error: None,
    }
}

/// The top-level `postman_extra` of a node's content: the original Postman fragment of the node.
fn top_extra(content: &mut NodeContent) -> &mut Extra {
    match content {
        NodeContent::Collection(collection) => &mut collection.postman_extra,
        NodeContent::Folder(folder) => &mut folder.postman_extra,
        NodeContent::Request(request) => &mut request.postman_extra,
    }
}

/// Merges the variables of a globals file into the stored set and saves it.
///
/// The stored set keeps its name and its Postman source object. While nothing is stored yet (no variables
/// and no source object) the file is taken whole, so that its source object is there for the export.
fn save_merged_globals(
    store: &mut Store,
    current: Environment,
    incoming: Environment,
    policy: MergePolicy,
) -> Result<(), String> {
    let variables = rqm_postman::merge_globals(&current.variables, &incoming.variables, policy);
    let base = if current.variables.is_empty() && current.postman_extra.is_empty() { incoming } else { current };
    store.save_environment(&Environment { variables, ..base }).map_err(message)
}
