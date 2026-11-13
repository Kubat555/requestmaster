//! Export to Postman files: [`AppCore`] builds the text and the suggested file name; choosing the place and
//! writing the file is the command's part (see `commands.rs`).

use rqm_model::{NodeKind, GLOBALS_ID};

use crate::core::{message, stored_environment, AppCore};

/// Characters Windows does not allow in a file name; each becomes `_` in a suggested name.
const FORBIDDEN_IN_FILE_NAME: [char; 9] = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

impl AppCore {
    /// The collection `id` as a Postman collection file: the suggested file name
    /// (`<name>.postman_collection.json`) and the JSON text. A node that is not a collection is an error.
    pub fn export_collection_text(&self, id: &str) -> Result<(String, String), String> {
        let nodes = {
            let store = self.store()?;
            if store.node(id).map_err(message)?.kind != NodeKind::Collection {
                return Err("not a collection".to_string());
            }
            store.subtree(id).map_err(message)?
        };
        let json = rqm_postman::export_collection(&nodes).map_err(message)?;
        // A subtree starts with its root: the collection itself.
        let name = nodes.first().map(|node| node.name.as_str()).unwrap_or_default();
        Ok((format!("{}.postman_collection.json", file_name_part(name)), pretty(&json)?))
    }

    /// The environment `id` as a Postman environment file (`<name>.postman_environment.json`), or the
    /// globals set for the id `globals` (`globals.postman_globals.json`): the suggested file name and the
    /// JSON text. An id that is not an environment is an error.
    pub fn export_environment_text(&self, id: &str) -> Result<(String, String), String> {
        let environment = stored_environment(&*self.store()?, id)?;
        let file_name = if id == GLOBALS_ID {
            "globals.postman_globals.json".to_string()
        } else {
            format!("{}.postman_environment.json", file_name_part(&environment.name))
        };
        Ok((file_name, pretty(&rqm_postman::export_environment(&environment))?))
    }
}

/// `name` with the characters a file name cannot hold replaced by `_`.
fn file_name_part(name: &str) -> String {
    name.chars().map(|ch| if FORBIDDEN_IN_FILE_NAME.contains(&ch) { '_' } else { ch }).collect()
}

/// JSON text indented with two spaces. This is the application's own output, not the user's text, so the
/// generic writer is fine here; the key order is the one the export built (`preserve_order`).
fn pretty(json: &serde_json::Value) -> Result<String, String> {
    serde_json::to_string_pretty(json).map_err(message)
}
