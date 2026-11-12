//! Merging an imported globals file into the globals set that already exists.
//!
//! Variables are matched by name. When a name is listed twice in one list, its last entry is the
//! one that counts: it is the one compared, replaced or added.

use std::collections::{HashMap, HashSet};

use rqm_model::Variable;

/// What to do with a variable whose value differs between the current set and the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergePolicy {
    /// The variable from the file replaces the current one entirely.
    TakeIncoming,
    /// The current variable stays as it is.
    KeepCurrent,
}

/// Index of the last entry of every name.
fn last_by_name(variables: &[Variable]) -> HashMap<&str, usize> {
    variables.iter().enumerate().map(|(index, variable)| (variable.key.as_str(), index)).collect()
}

/// For every name of `incoming`, once and in `incoming` order: the entry that counts
/// and the position of the current variable with that name, if there is one.
fn matched<'a>(current: &'a [Variable], incoming: &'a [Variable]) -> Vec<(&'a Variable, Option<usize>)> {
    let current_last = last_by_name(current);
    let incoming_last = last_by_name(incoming);
    let mut seen = HashSet::new();
    incoming
        .iter()
        .filter(|variable| seen.insert(variable.key.as_str()))
        .map(|variable| {
            let name = variable.key.as_str();
            (&incoming[incoming_last[name]], current_last.get(name).copied())
        })
        .collect()
}

/// Names present in both lists with different values (enabled or not), in `incoming` order.
pub fn globals_conflicts(current: &[Variable], incoming: &[Variable]) -> Vec<String> {
    matched(current, incoming)
        .into_iter()
        .filter(|(variable, at)| at.is_some_and(|at| current[at].value != variable.value))
        .map(|(variable, _)| variable.key.clone())
        .collect()
}

/// Merges the variables of a globals file into the current ones.
///
/// Current order is kept, new variables are appended in `incoming` order. A variable with the
/// same name and value stays the current one entirely; a conflict is settled by `policy`.
pub fn merge_globals(current: &[Variable], incoming: &[Variable], policy: MergePolicy) -> Vec<Variable> {
    let mut merged = current.to_vec();
    for (variable, at) in matched(current, incoming) {
        match at {
            Some(at) => {
                if policy == MergePolicy::TakeIncoming && merged[at].value != variable.value {
                    merged[at] = variable.clone();
                }
            }
            None => merged.push(variable.clone()),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use rqm_model::Extra;

    fn var(key: &str, value: &str, secret: bool) -> Variable {
        Variable { key: key.into(), value: value.into(), secret, enabled: true, postman_extra: Extra::new() }
    }

    #[test]
    fn last_entry_of_a_repeated_name_counts() {
        let current = vec![var("a", "old", false), var("a", "1", false)];
        let incoming = vec![var("a", "x", false), var("n", "first", false), var("a", "1", true), var("n", "last", false)];
        // the last `a` of both lists has the same value: no conflict, the current one stays entirely
        assert_eq!(globals_conflicts(&current, &incoming), Vec::<String>::new());
        let merged = merge_globals(&current, &incoming, MergePolicy::TakeIncoming);
        assert_eq!(merged, vec![var("a", "old", false), var("a", "1", false), var("n", "last", false)]);
    }

    #[test]
    fn taken_variable_replaces_the_current_one_entirely() {
        let current = vec![var("a", "1", false)];
        let mut incoming = vec![var("a", "2", true)];
        incoming[0].enabled = false;
        assert_eq!(globals_conflicts(&current, &incoming), vec!["a".to_string()]);
        assert_eq!(merge_globals(&current, &incoming, MergePolicy::TakeIncoming), incoming);
        assert_eq!(merge_globals(&current, &incoming, MergePolicy::KeepCurrent), current);
    }
}
