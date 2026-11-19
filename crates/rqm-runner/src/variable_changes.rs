//! What the scripts of a send changed in the stored variables, written to the store: one rule for globals,
//! the environment and the collection.

use rqm_model::{Extra, NodeContent, Variable, GLOBALS_ID};
use rqm_store::{Store, StoreError};

use crate::{StoredScope, VariableChange, VariableOp};

/// A change of a script that reached the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedChange {
    pub scope: StoredScope,
    /// `"globals"`, the id of the environment or the id of the collection node.
    pub target_id: String,
    pub op: VariableOp,
}

/// The one rule of a script change on a stored list of variables. A script sees the enabled rows only, so
/// a disabled row is never changed: `Set` writes the value of the last enabled row of the name (or adds a
/// row), `Unset` removes the enabled rows of the name, `Clear` — all enabled rows.
pub fn apply_to_variables(variables: &mut Vec<Variable>, op: &VariableOp) {
    match op {
        VariableOp::Set { key, value } => {
            match variables.iter_mut().rev().find(|variable| variable.enabled && variable.key == *key) {
                Some(variable) => variable.value.clone_from(value),
                None => variables.push(Variable {
                    key: key.clone(),
                    value: value.clone(),
                    secret: false,
                    enabled: true,
                    postman_extra: Extra::new(),
                }),
            }
        }
        VariableOp::Unset { key } => variables.retain(|variable| !(variable.enabled && variable.key == *key)),
        VariableOp::Clear => variables.retain(|variable| !variable.enabled),
    }
}

/// Applies the operations of `scope` to `variables`; `true` when the list is not what it was.
fn apply_scope(variables: &mut Vec<Variable>, changes: &[VariableChange], scope: StoredScope) -> bool {
    let before = variables.clone();
    for change in changes.iter().filter(|change| change.scope == scope) {
        apply_to_variables(variables, &change.op);
    }
    *variables != before
}

/// Applies the changes over what the store holds now and returns what was applied, in the order of `changes`.
///
/// Each scope that has changes is read anew, changed and written back only when its list became different;
/// of a collection node only `variables` change. Without `environment_id` the changes of the environment
/// are left out; so are those of an environment or a collection that is no longer in the store — it is not
/// created again. The globals row is created when there is none. The scopes are not written in one
/// transaction: the caller holds the lock of the store for the whole call, and after a failure of the
/// store the scopes written before it stay written.
pub fn apply_variable_changes(
    store: &mut Store,
    collection_id: &str,
    environment_id: Option<&str>,
    changes: &[VariableChange],
) -> Result<Vec<AppliedChange>, StoreError> {
    let has = |scope: StoredScope| changes.iter().any(|change| change.scope == scope);
    // The target of each scope; `None` — there is none, the changes of the scope are left out.
    let mut globals_target = None;
    let mut collection_target = None;
    let mut environment_target = None;

    if has(StoredScope::Globals) {
        let mut globals = store.globals()?;
        if apply_scope(&mut globals.variables, changes, StoredScope::Globals) {
            store.save_environment(&globals)?;
        }
        globals_target = Some(GLOBALS_ID.to_string());
    }
    if has(StoredScope::Collection) {
        let node = match store.node(collection_id) {
            Ok(node) => Some(node),
            Err(StoreError::NotFound(_)) => None,
            Err(error) => return Err(error),
        };
        if let Some(NodeContent::Collection(mut content)) = node.map(|node| node.content) {
            if apply_scope(&mut content.variables, changes, StoredScope::Collection) {
                store.update_content(collection_id, &NodeContent::Collection(content))?;
            }
            collection_target = Some(collection_id.to_string());
        }
    }
    if let Some(id) = environment_id.filter(|_| has(StoredScope::Environment)) {
        let stored = store.environments()?.into_iter().find(|environment| environment.id == id);
        if let Some(mut environment) = stored {
            if apply_scope(&mut environment.variables, changes, StoredScope::Environment) {
                store.save_environment(&environment)?;
            }
            environment_target = Some(id.to_string());
        }
    }

    let applied = changes.iter().filter_map(|change| {
        let target = match change.scope {
            StoredScope::Globals => &globals_target,
            StoredScope::Collection => &collection_target,
            StoredScope::Environment => &environment_target,
        };
        let target_id = target.clone()?;
        Some(AppliedChange { scope: change.scope, target_id, op: change.op.clone() })
    });
    Ok(applied.collect())
}
