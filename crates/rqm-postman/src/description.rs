//! The description of a collection and of a folder. It has no field in the model: it stays where Postman
//! keeps it, in the source object of the node (`postman_extra`), so the export writes it back as it is.

use rqm_model::{new_id, Extra, NodeKind};
use serde_json::Value;

use crate::export::SCHEMA_V21;

/// Key of the description in the source object of a folder and in `info` of a collection.
const DESCRIPTION: &str = "description";
/// Key of the text in the object form of a description: `{"content": "...", "type": "text/markdown"}`.
const CONTENT: &str = "content";
/// Key of the object of a collection that holds its description.
const INFO: &str = "info";

/// The description kept in the Postman source object of a collection (`info.description`) or of a folder
/// (`description`); empty when there is none. A request has none here.
pub fn description_of(kind: NodeKind, extra: &Extra) -> String {
    let holder = match kind {
        NodeKind::Collection => extra.get(INFO).and_then(Value::as_object),
        NodeKind::Folder => Some(extra),
        NodeKind::Request => None,
    };
    holder.map(read).unwrap_or_default()
}

/// Writes the description into the source object; a text equal to the current one changes nothing.
///
/// A description kept as an object gets only its `content` replaced; any other becomes a string. A
/// collection whose source object has no `info` (created in the application) gets one with a new
/// `_postman_id` and the v2.1 schema, the same the export writes. Nothing is written for a request.
pub fn set_description(kind: NodeKind, extra: &mut Extra, text: &str) {
    if description_of(kind, extra) == text {
        return;
    }
    match kind {
        NodeKind::Collection => {
            if !extra.get(INFO).is_some_and(Value::is_object) {
                let mut info = Extra::new();
                info.insert("_postman_id".to_string(), Value::from(new_id()));
                info.insert("schema".to_string(), Value::from(SCHEMA_V21));
                extra.insert(INFO.to_string(), Value::Object(info));
            }
            if let Some(Value::Object(info)) = extra.get_mut(INFO) {
                write(info, text);
            }
        }
        NodeKind::Folder => write(extra, text),
        NodeKind::Request => {}
    }
}

/// The text of the description of `holder`: a string is itself, an object gives its string `content`,
/// anything else (`null`, no key, another type) is empty.
fn read(holder: &Extra) -> String {
    match holder.get(DESCRIPTION) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Object(object)) => object.get(CONTENT).and_then(Value::as_str).unwrap_or_default().to_string(),
        _ => String::new(),
    }
}

/// Puts `text` into `holder`, keeping the object form when the description has it.
fn write(holder: &mut Extra, text: &str) {
    match holder.get_mut(DESCRIPTION) {
        Some(Value::Object(object)) if object.get(CONTENT).is_some_and(Value::is_string) => {
            object.insert(CONTENT.to_string(), Value::from(text));
        }
        _ => {
            holder.insert(DESCRIPTION.to_string(), Value::from(text));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn extra(value: Value) -> Extra {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn reads_the_three_forms() {
        // A string is the text itself.
        assert_eq!(description_of(NodeKind::Folder, &extra(json!({"description": "plain"}))), "plain");
        assert_eq!(description_of(NodeKind::Collection, &extra(json!({"info": {"description": "plain"}}))), "plain");
        // An object gives its `content`.
        let object = json!({"description": {"content": "in object", "type": "text/markdown"}});
        assert_eq!(description_of(NodeKind::Folder, &extra(object.clone())), "in object");
        assert_eq!(description_of(NodeKind::Collection, &extra(json!({"info": object}))), "in object");
        // `null`, no key and anything else are empty.
        for none in [json!({"description": null}), json!({}), json!({"description": 5}), json!({"description": {"content": 5}})] {
            assert_eq!(description_of(NodeKind::Folder, &extra(none.clone())), "");
            assert_eq!(description_of(NodeKind::Collection, &extra(json!({"info": none}))), "");
        }
        assert_eq!(description_of(NodeKind::Collection, &extra(json!({"description": "not in info"}))), "");
        assert_eq!(description_of(NodeKind::Collection, &extra(json!({"info": "not an object"}))), "");
        // A request has no description here.
        assert_eq!(description_of(NodeKind::Request, &extra(json!({"description": "of a request"}))), "");
    }

    #[test]
    fn writing_the_same_text_changes_nothing() {
        for source in [
            json!({"description": null}),
            json!({"description": 5}),
            json!({"description": {"type": "text/markdown"}}),
            json!({"description": {"content": "same", "type": "text/markdown"}}),
            json!({"description": "same"}),
        ] {
            let text = description_of(NodeKind::Folder, &extra(source.clone()));
            let mut folder = extra(source.clone());
            set_description(NodeKind::Folder, &mut folder, &text);
            assert_eq!(Value::Object(folder), source);

            let source = json!({"info": source, "item": []});
            let mut collection = extra(source.clone());
            set_description(NodeKind::Collection, &mut collection, &text);
            assert_eq!(Value::Object(collection), source);
        }
    }

    #[test]
    fn editing_the_object_form_changes_only_content() {
        let mut folder = extra(json!({"description": {"content": "old", "type": "text/markdown", "version": 2}, "id": "f"}));
        set_description(NodeKind::Folder, &mut folder, "new");
        assert_eq!(
            Value::Object(folder),
            json!({"description": {"content": "new", "type": "text/markdown", "version": 2}, "id": "f"})
        );

        let mut collection = extra(json!({"info": {"_postman_id": "p", "description": {"content": "old", "type": "text/plain"}}}));
        set_description(NodeKind::Collection, &mut collection, "");
        assert_eq!(
            Value::Object(collection),
            json!({"info": {"_postman_id": "p", "description": {"content": "", "type": "text/plain"}}})
        );
    }

    #[test]
    fn any_other_form_becomes_a_string() {
        for source in [json!({}), json!({"description": null}), json!({"description": "old"}), json!({"description": {"type": "t"}})] {
            let mut folder = extra(source);
            set_description(NodeKind::Folder, &mut folder, "new");
            assert_eq!(Value::Object(folder), json!({"description": "new"}));
        }
        let mut collection = extra(json!({"info": {"_postman_id": "p", "name": "C", "schema": "s"}, "variable": []}));
        set_description(NodeKind::Collection, &mut collection, "new");
        assert_eq!(
            Value::Object(collection),
            json!({"info": {"_postman_id": "p", "name": "C", "schema": "s", "description": "new"}, "variable": []})
        );
        // A string is cleared to an empty string.
        let mut folder = extra(json!({"description": "old"}));
        set_description(NodeKind::Folder, &mut folder, "");
        assert_eq!(Value::Object(folder), json!({"description": ""}));
    }

    #[test]
    fn empty_text_does_not_create_the_key() {
        let mut folder = Extra::new();
        set_description(NodeKind::Folder, &mut folder, "");
        assert!(folder.is_empty());

        let mut created_here = Extra::new();
        set_description(NodeKind::Collection, &mut created_here, "");
        assert!(created_here.is_empty());

        let mut collection = extra(json!({"info": {"name": "C"}}));
        set_description(NodeKind::Collection, &mut collection, "");
        assert_eq!(Value::Object(collection), json!({"info": {"name": "C"}}));
    }

    #[test]
    fn collection_created_here_gets_info_with_an_id_and_the_schema() {
        let mut collection = Extra::new();
        set_description(NodeKind::Collection, &mut collection, "about");
        let info = collection["info"].as_object().unwrap();
        assert_eq!(info["description"], "about");
        assert_eq!(info["schema"], SCHEMA_V21);
        assert!(info["_postman_id"].as_str().is_some_and(|id| !id.is_empty()));
        assert_eq!(info.len(), 3);
    }

    #[test]
    fn request_is_never_written() {
        let mut request = extra(json!({"request": "https://example.test"}));
        set_description(NodeKind::Request, &mut request, "text");
        assert_eq!(Value::Object(request), json!({"request": "https://example.test"}));
    }
}
