//! The collections tree: the `node` table.
//!
//! The helpers take a plain `&Connection`, so the same code runs on the store's connection and inside a
//! transaction. Siblings are always numbered 0..n by `sort_order`.

use std::collections::HashMap;

use rqm_model::{new_id, Node, NodeContent, NodeId, NodeKind};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::{Store, StoreError};

/// Columns of a full node row, in the order [`RawNode::from_row`] reads them.
const NODE_COLUMNS: &str = "id, parent_id, kind, name, sort_order, content";

/// A node as the tree shows it: everything except the content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    /// Id of the node.
    pub id: NodeId,
    /// `None` for a collection.
    pub parent_id: Option<NodeId>,
    /// Kind of the node.
    pub kind: NodeKind,
    /// Name shown in the tree.
    pub name: String,
    /// Position among the siblings, 0..n.
    pub sort_order: i64,
    /// HTTP method of a request, taken from its content (`$.method`); `None` for folders and collections.
    pub method: Option<String>,
}

/// A row of `node` before its kind and content are parsed.
struct RawNode {
    id: String,
    parent_id: Option<String>,
    kind: String,
    name: String,
    sort_order: i64,
    content: String,
}

impl RawNode {
    /// Reads a row selected with [`NODE_COLUMNS`].
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(RawNode {
            id: row.get(0)?,
            parent_id: row.get(1)?,
            kind: row.get(2)?,
            name: row.get(3)?,
            sort_order: row.get(4)?,
            content: row.get(5)?,
        })
    }

    /// Parses the kind and the content JSON into the model.
    fn into_node(self) -> Result<Node, StoreError> {
        let kind = parse_kind(&self.id, &self.kind)?;
        let content = NodeContent::from_json(kind, &self.content)
            .map_err(|e| StoreError::CorruptContent { id: self.id.clone(), error: e.to_string() })?;
        Ok(Node { id: self.id, parent_id: self.parent_id, kind, name: self.name, sort_order: self.sort_order, content })
    }
}

/// Text of a kind in the `kind` column.
fn kind_text(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Collection => "collection",
        NodeKind::Folder => "folder",
        NodeKind::Request => "request",
    }
}

/// Reads the `kind` column of the node `id`.
fn parse_kind(id: &str, text: &str) -> Result<NodeKind, StoreError> {
    match text {
        "collection" => Ok(NodeKind::Collection),
        "folder" => Ok(NodeKind::Folder),
        "request" => Ok(NodeKind::Request),
        other => Err(StoreError::CorruptContent { id: id.to_string(), error: format!("unknown node kind `{other}`") }),
    }
}

/// The node with its content; `NotFound` when there is no such id.
fn read_node(conn: &Connection, id: &str) -> Result<Node, StoreError> {
    conn.query_row(&format!("SELECT {NODE_COLUMNS} FROM node WHERE id = ?1"), [id], RawNode::from_row)
        .optional()?
        .ok_or_else(|| StoreError::NotFound(id.to_string()))?
        .into_node()
}

/// Kind and parent of the node; `NotFound` when there is no such id.
fn kind_and_parent(conn: &Connection, id: &str) -> Result<(NodeKind, Option<NodeId>), StoreError> {
    let (kind, parent_id): (String, Option<NodeId>) = conn
        .query_row("SELECT kind, parent_id FROM node WHERE id = ?1", [id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?
        .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
    Ok((parse_kind(id, &kind)?, parent_id))
}

/// Checks that a node of `kind` may be placed under `parent` (`None` = as a root):
/// a collection is a root and only a collection is; a request has no children.
fn check_placement(conn: &Connection, kind: NodeKind, parent: Option<&str>) -> Result<(), StoreError> {
    match (kind, parent) {
        (NodeKind::Collection, None) => Ok(()),
        (NodeKind::Collection, Some(_)) => {
            Err(StoreError::InvalidMove("a collection can only be a root".to_string()))
        }
        (_, None) => Err(StoreError::InvalidMove("only a collection can be a root".to_string())),
        (_, Some(parent)) => match kind_and_parent(conn, parent)?.0 {
            NodeKind::Request => Err(StoreError::InvalidMove("a request cannot have children".to_string())),
            NodeKind::Collection | NodeKind::Folder => Ok(()),
        },
    }
}

/// Ids of the children of `parent` (`None` = the collections) in their order.
fn sibling_ids(conn: &Connection, parent: Option<&str>) -> Result<Vec<NodeId>, StoreError> {
    let mut statement = conn.prepare("SELECT id FROM node WHERE parent_id IS ?1 ORDER BY sort_order, rowid")?;
    let ids = statement.query_map([parent], |row| row.get(0))?.collect::<rusqlite::Result<Vec<NodeId>>>()?;
    Ok(ids)
}

/// Writes `sort_order` 0..n to the nodes in the given order.
fn renumber(conn: &Connection, ids: &[NodeId]) -> Result<(), StoreError> {
    let mut statement = conn.prepare("UPDATE node SET sort_order = ?1 WHERE id = ?2")?;
    for (index, id) in ids.iter().enumerate() {
        // A sibling list never comes close to `i64::MAX` entries, so the cast is lossless.
        statement.execute(params![index as i64, id])?;
    }
    Ok(())
}

/// `sort_order` for a node appended after the last child of `parent` (`None` = after the last collection).
fn next_sort_order(conn: &Connection, parent: Option<&str>) -> Result<i64, StoreError> {
    Ok(conn.query_row("SELECT coalesce(max(sort_order) + 1, 0) FROM node WHERE parent_id IS ?1", [parent], |row| {
        row.get(0)
    })?)
}

/// Inserts one row of `node`.
fn insert_row(
    conn: &Connection,
    id: &str,
    parent_id: Option<&str>,
    kind: NodeKind,
    name: &str,
    sort_order: i64,
    content: &NodeContent,
) -> Result<(), StoreError> {
    conn.execute(
        &format!("INSERT INTO node ({NODE_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"),
        params![id, parent_id, kind_text(kind), name, sort_order, content.to_json()],
    )?;
    Ok(())
}

/// The node `root_id` with all its descendants: root first, every parent before its children,
/// siblings by `sort_order`.
fn read_subtree(conn: &Connection, root_id: &str) -> Result<Vec<Node>, StoreError> {
    let mut statement = conn.prepare(&format!(
        "WITH RECURSIVE sub (id) AS (
             SELECT id FROM node WHERE id = ?1
             UNION ALL
             SELECT node.id FROM node JOIN sub ON node.parent_id = sub.id
         )
         SELECT {NODE_COLUMNS} FROM node WHERE id IN (SELECT id FROM sub) ORDER BY sort_order, rowid"
    ))?;
    let mut root = None;
    let mut children: HashMap<NodeId, Vec<Node>> = HashMap::new();
    for raw in statement.query_map([root_id], RawNode::from_row)? {
        let node = raw?.into_node()?;
        match &node.parent_id {
            Some(parent_id) if node.id != root_id => children.entry(parent_id.clone()).or_default().push(node),
            _ => root = Some(node),
        }
    }
    let root = root.ok_or_else(|| StoreError::NotFound(root_id.to_string()))?;
    // Depth-first walk: a node is written out, then its children are put on the stack in reverse,
    // so the first child is taken next.
    let mut ordered = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Some(own_children) = children.remove(&node.id) {
            stack.extend(own_children.into_iter().rev());
        }
        ordered.push(node);
    }
    Ok(ordered)
}

impl Store {
    /// Inserts the nodes in one transaction; used by import. Parents must precede children. A root node
    /// (collection) ignores its own `sort_order` and becomes the last collection.
    pub fn insert_nodes(&mut self, nodes: &[Node]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        for node in nodes {
            let sort_order = match node.parent_id {
                None => next_sort_order(&tx, None)?,
                Some(_) => node.sort_order,
            };
            insert_row(&tx, &node.id, node.parent_id.as_deref(), node.kind, &node.name, sort_order, &node.content)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// All nodes without content, ordered by (parent_id, sort_order).
    pub fn tree(&self) -> Result<Vec<TreeRow>, StoreError> {
        let mut statement = self.conn.prepare(
            "SELECT id, parent_id, kind, name, sort_order, json_extract(content, '$.method') \
             FROM node ORDER BY parent_id, sort_order, rowid",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;
        let mut tree = Vec::new();
        for row in rows {
            let (id, parent_id, kind, name, sort_order, method) = row?;
            let kind = parse_kind(&id, &kind)?;
            tree.push(TreeRow { id, parent_id, kind, name, sort_order, method });
        }
        Ok(tree)
    }

    /// The node with its content; `NotFound` when there is no such id.
    pub fn node(&self, id: &str) -> Result<Node, StoreError> {
        read_node(&self.conn, id)
    }

    /// The node with all its descendants. Root first, every parent before its children, siblings by sort_order.
    pub fn subtree(&self, root_id: &str) -> Result<Vec<Node>, StoreError> {
        read_subtree(&self.conn, root_id)
    }

    /// Ancestors of the node, from the collection down to the direct parent; empty for a collection.
    pub fn ancestors(&self, id: &str) -> Result<Vec<Node>, StoreError> {
        let mut ancestors = Vec::new();
        let mut parent_id = kind_and_parent(&self.conn, id)?.1;
        while let Some(current) = parent_id {
            let node = read_node(&self.conn, &current)?;
            parent_id = node.parent_id.clone();
            ancestors.push(node);
        }
        ancestors.reverse();
        Ok(ancestors)
    }

    /// Creates a node with a fresh id and appends it as the last child of `parent_id` (or as the last
    /// collection); returns the new id. `InvalidMove` when the tree rules forbid this parent.
    pub fn create_node(
        &mut self,
        parent_id: Option<&str>,
        kind: NodeKind,
        name: &str,
        content: &NodeContent,
    ) -> Result<NodeId, StoreError> {
        let tx = self.conn.transaction()?;
        check_placement(&tx, kind, parent_id)?;
        let id = new_id();
        let sort_order = next_sort_order(&tx, parent_id)?;
        insert_row(&tx, &id, parent_id, kind, name, sort_order, content)?;
        tx.commit()?;
        Ok(id)
    }

    /// Changes the name shown in the tree; `NotFound` when there is no such id.
    pub fn rename_node(&mut self, id: &str, name: &str) -> Result<(), StoreError> {
        match self.conn.execute("UPDATE node SET name = ?1 WHERE id = ?2", [name, id])? {
            0 => Err(StoreError::NotFound(id.to_string())),
            _ => Ok(()),
        }
    }

    /// Replaces the content of the node; `NotFound` when there is no such id.
    pub fn update_content(&mut self, id: &str, content: &NodeContent) -> Result<(), StoreError> {
        match self.conn.execute("UPDATE node SET content = ?1 WHERE id = ?2", [content.to_json().as_str(), id])? {
            0 => Err(StoreError::NotFound(id.to_string())),
            _ => Ok(()),
        }
    }

    /// Moves under `new_parent` at position `index` (clamped) and renumbers siblings 0..n in both parents.
    /// `InvalidMove` when the tree rules forbid it: a request has no children, a collection is a root and only
    /// a collection is, a node cannot go into itself or into its own descendant.
    pub fn move_node(&mut self, id: &str, new_parent: Option<&str>, index: usize) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        let (kind, old_parent) = kind_and_parent(&tx, id)?;
        check_placement(&tx, kind, new_parent)?;
        // Walk from the new parent up to its collection: meeting the moved node means the target is
        // the node itself or lies inside it.
        let mut cursor = new_parent.map(str::to_string);
        while let Some(current) = cursor {
            if current == id {
                return Err(StoreError::InvalidMove("a node cannot be moved into itself or its descendant".to_string()));
            }
            cursor = kind_and_parent(&tx, &current)?.1;
        }

        let mut siblings = sibling_ids(&tx, new_parent)?;
        siblings.retain(|sibling| sibling != id);
        siblings.insert(index.min(siblings.len()), id.to_string());
        tx.execute("UPDATE node SET parent_id = ?1 WHERE id = ?2", params![new_parent, id])?;
        renumber(&tx, &siblings)?;
        if old_parent.as_deref() != new_parent {
            renumber(&tx, &sibling_ids(&tx, old_parent.as_deref())?)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Deletes the node with its descendants and renumbers the remaining siblings 0..n (one transaction).
    pub fn delete_node(&mut self, id: &str) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        let (_, parent_id) = kind_and_parent(&tx, id)?;
        // Descendants go with the node: `parent_id` is declared ON DELETE CASCADE.
        tx.execute("DELETE FROM node WHERE id = ?1", [id])?;
        renumber(&tx, &sibling_ids(&tx, parent_id.as_deref())?)?;
        tx.commit()?;
        Ok(())
    }

    /// Deep copy with fresh ids right after the original; the copy's name gets the suffix " Copy".
    /// Returns the id of the copy.
    pub fn duplicate_node(&mut self, id: &str) -> Result<NodeId, StoreError> {
        let tx = self.conn.transaction()?;
        let originals = read_subtree(&tx, id)?;
        let new_ids: HashMap<&str, NodeId> = originals.iter().map(|node| (node.id.as_str(), new_id())).collect();
        // The first node is the copied one: it stays under the same parent, the rest follow their copied parents.
        let root = &originals[0];
        let copy_id = new_ids[root.id.as_str()].clone();
        let mut siblings = sibling_ids(&tx, root.parent_id.as_deref())?;
        let after_original = siblings.iter().position(|sibling| sibling == id).map_or(siblings.len(), |at| at + 1);
        siblings.insert(after_original, copy_id.clone());

        let copy_name = format!("{} Copy", root.name);
        insert_row(&tx, &copy_id, root.parent_id.as_deref(), root.kind, &copy_name, root.sort_order, &root.content)?;
        for node in &originals[1..] {
            // Parents precede children in `originals`, so the copied parent is already inserted.
            let parent_id = node.parent_id.as_deref().and_then(|parent| new_ids.get(parent));
            insert_row(
                &tx,
                &new_ids[node.id.as_str()],
                parent_id.map(String::as_str),
                node.kind,
                &node.name,
                node.sort_order,
                &node.content,
            )?;
        }
        renumber(&tx, &siblings)?;
        tx.commit()?;
        Ok(copy_id)
    }
}
