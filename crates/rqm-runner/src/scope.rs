//! The variables around a node: the three stored scopes, the resolver of a send built from them, and the
//! list of what `{{name}}` means at that node for the highlighting in the window.

use rqm_model::{NodeContent, Variable};
use rqm_store::{Store, StoreError};
use rqm_vars::{Resolver, Scope, ScopeKind, DYNAMIC_NAMES};

use crate::{environment_by_id, SendInput};

/// Stands in for the value of every secret variable when [`variables_in_scope`] looks for the names whose
/// value depends on a secret. The NUL characters keep it apart from anything a user types.
const SECRET_MARK: &str = "\u{0}secret\u{0}";

/// The variables of the three stored scopes around a node, disabled ones included.
#[derive(Debug, Clone)]
pub struct ScopeSources {
    /// Variables of the globals set.
    pub globals: Vec<Variable>,
    /// Variables of the collection the node belongs to (or is).
    pub collection: Vec<Variable>,
    /// Variables of the chosen environment; empty when none is chosen.
    pub environment: Vec<Variable>,
}

impl ScopeSources {
    /// The scopes of a send: the collection is the first collection among the ancestors of the request.
    pub fn of_input(input: &SendInput) -> Self {
        let collection = input.ancestors.iter().find_map(|node| collection_variables(&node.content));
        ScopeSources {
            globals: input.globals.variables.clone(),
            collection: collection.cloned().unwrap_or_default(),
            environment: input.environment.as_ref().map(|environment| environment.variables.clone()).unwrap_or_default(),
        }
    }

    /// `node_id` is a request, a folder or a collection: the collection is its first ancestor or the node itself.
    /// An unknown node or environment is `StoreError::NotFound`.
    pub fn load(store: &Store, node_id: &str, environment_id: Option<&str>) -> Result<Self, StoreError> {
        let node = store.node(node_id)?;
        let collection = match node.content {
            NodeContent::Collection(content) => content.variables,
            _ => {
                let ancestors = store.ancestors(node_id)?;
                let variables = ancestors.iter().find_map(|ancestor| collection_variables(&ancestor.content));
                variables.cloned().unwrap_or_default()
            }
        };
        let environment = match environment_id {
            Some(id) => environment_by_id(store, id)?.variables,
            None => Vec::new(),
        };
        Ok(ScopeSources { globals: store.globals()?.variables, collection, environment })
    }

    /// Enabled variables only, the widest scope first — the resolver of a send.
    /// The local scope is empty at this stage (it is filled by scripts).
    pub fn resolver(&self) -> Resolver {
        self.resolver_with(|variable| variable.value.clone())
    }

    /// The resolver of [`ScopeSources::resolver`] with `value(variable)` in place of every value.
    fn resolver_with(&self, value: impl Fn(&Variable) -> String) -> Resolver {
        let enabled = |variables: &[Variable]| -> Vec<(String, String)> {
            let enabled = variables.iter().filter(|variable| variable.enabled);
            enabled.map(|variable| (variable.key.clone(), value(variable))).collect()
        };
        Resolver::new(vec![
            Scope { kind: ScopeKind::Global, values: enabled(&self.globals) },
            Scope { kind: ScopeKind::Collection, values: enabled(&self.collection) },
            Scope { kind: ScopeKind::Environment, values: enabled(&self.environment) },
        ])
    }
}

/// The variables of a collection node; `None` for a folder and a request.
fn collection_variables(content: &NodeContent) -> Option<&Vec<Variable>> {
    match content {
        NodeContent::Collection(content) => Some(&content.variables),
        _ => None,
    }
}

/// Where the value of a `{{name}}` comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableOrigin {
    /// The globals set.
    Global,
    /// The collection of the node.
    Collection,
    /// The chosen environment.
    Environment,
    /// Not a stored variable: `{{$guid}}` and the like, a new value on every occurrence at the send.
    Dynamic,
}

/// One name that `{{name}}` resolves at a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeVariable {
    /// The name without braces; a dynamic one starts with `$`.
    pub name: String,
    /// The scope that wins for this name.
    pub origin: VariableOrigin,
    /// The value after the full substitution; `None` for a secret and for a dynamic name.
    pub value: Option<String>,
    /// `true` — the variable is secret or its value refers to a secret one.
    pub secret: bool,
}

/// What `{{name}}` means with these scopes: one entry per name of an enabled variable, sorted by name, then
/// the dynamic names no variable shadows, in the order of [`DYNAMIC_NAMES`].
///
/// A value is what a send would put in place of `{{name}}`, except that dynamic tokens inside it stay as
/// typed. The value of a secret variable, and any value a secret one takes part in, is never returned.
pub fn variables_in_scope(sources: &ScopeSources) -> Vec<ScopeVariable> {
    let resolver = sources.resolver();
    // The same scopes with every secret value replaced: a name that resolves differently here depends on a secret.
    let masked = sources.resolver_with(|variable| {
        if variable.secret {
            SECRET_MARK.to_string()
        } else {
            variable.value.clone()
        }
    });
    // What stands in place of `{{name}}`; dynamic tokens are left as typed.
    let resolved =
        |resolver: &Resolver, name: &str| resolver.substitute_with(&format!("{{{{{name}}}}}"), &mut |_| None).text;

    let mut variables: Vec<ScopeVariable> = resolver
        .entries()
        .into_iter()
        .map(|(name, _, kind)| {
            let value = resolved(&resolver, name);
            let secret = value != resolved(&masked, name);
            let origin = match kind {
                ScopeKind::Global => VariableOrigin::Global,
                ScopeKind::Collection => VariableOrigin::Collection,
                // `ScopeSources::resolver` has no local scope.
                ScopeKind::Environment | ScopeKind::Local => VariableOrigin::Environment,
            };
            ScopeVariable { name: name.to_string(), origin, value: (!secret).then_some(value), secret }
        })
        .collect();
    let dynamic = DYNAMIC_NAMES.iter().map(|name| format!("${name}")).filter(|name| resolver.lookup(name).is_none());
    variables.extend(dynamic.map(|name| ScopeVariable {
        name,
        origin: VariableOrigin::Dynamic,
        value: None,
        secret: false,
    }));
    variables
}
