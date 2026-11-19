//! The consent to run scripts, kept per collection in the store's `app_state` table.

use rqm_model::NodeKind;
use rqm_store::{Store, StoreError};

/// Key of `app_state`: a JSON array with the ids of the collections whose scripts may run.
pub const SCRIPTS_ALLOWED_KEY: &str = "scripts_allowed";

/// The stored list; a value that is not an array of strings is an empty list.
fn allowed_ids(store: &Store) -> Result<Vec<String>, StoreError> {
    let value = store.state_get(SCRIPTS_ALLOWED_KEY)?;
    Ok(value.and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default())
}

fn write_ids(store: &mut Store, ids: &[String]) -> Result<(), StoreError> {
    store.state_set(SCRIPTS_ALLOWED_KEY, &serde_json::Value::from(ids).to_string())
}

/// The id of the collection of the node: the node itself or its first ancestor. Unknown node — `NotFound`.
pub fn collection_of(store: &Store, node_id: &str) -> Result<String, StoreError> {
    // `ancestors` answers `NotFound` for an unknown node and nothing for a collection.
    match store.ancestors(node_id)?.into_iter().next() {
        Some(collection) => Ok(collection.id),
        None => Ok(node_id.to_string()),
    }
}

/// Whether the scripts of the collection may run.
pub fn scripts_allowed(store: &Store, collection_id: &str) -> Result<bool, StoreError> {
    Ok(allowed_ids(store)?.iter().any(|id| id == collection_id))
}

/// Gives or takes back the consent of the collection; the stored list holds an id once.
pub fn set_scripts_allowed(store: &mut Store, collection_id: &str, allowed: bool) -> Result<(), StoreError> {
    let mut ids: Vec<String> = Vec::new();
    for id in allowed_ids(store)? {
        if id != collection_id && !ids.contains(&id) {
            ids.push(id);
        }
    }
    if allowed {
        ids.push(collection_id.to_string());
    }
    write_ids(store, &ids)
}

/// Drops the ids of the collections that are no longer in the tree.
pub fn forget_deleted_collections(store: &mut Store) -> Result<(), StoreError> {
    let ids = allowed_ids(store)?;
    let mut kept = Vec::with_capacity(ids.len());
    for id in &ids {
        match store.node(id) {
            Ok(node) if node.kind == NodeKind::Collection => kept.push(id.clone()),
            Ok(_) | Err(StoreError::NotFound(_)) => {}
            Err(error) => return Err(error),
        }
    }
    if kept == ids {
        return Ok(());
    }
    write_ids(store, &kept)
}
