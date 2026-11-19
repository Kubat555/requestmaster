//! The consent to run the scripts of a collection: the `scripts_allowed` list and what a send makes of it.

use rqm_model::*;
use rqm_runner::*;
use rqm_store::{Store, StoreError};

struct Tree {
    store: Store,
    col: String,
    folder: String,
    req: String,
}

/// Collection `Col` → folder `F` → request `Req`.
fn tree() -> Tree {
    let mut store = Store::open_in_memory().unwrap();
    let col = collection(&mut store, "Col");
    let folder_content =
        NodeContent::Folder(FolderContent { auth: None, scripts: Scripts::default(), postman_extra: Extra::new() });
    let folder = store.create_node(Some(&col), NodeKind::Folder, "F", &folder_content).unwrap();
    let request = RequestContent { method: "GET".into(), url: "http://127.0.0.1:1/x".into(), query: vec![],
        path_variables: vec![], headers: vec![], body: Body::None, auth: None, scripts: Scripts::default(),
        settings: RequestSettings::default(), disabled_auto_headers: vec![], postman_extra: Extra::new() };
    let req = store.create_node(Some(&folder), NodeKind::Request, "Req", &NodeContent::Request(request)).unwrap();
    Tree { store, col, folder, req }
}
fn collection(store: &mut Store, name: &str) -> String {
    let content =
        CollectionContent { auth: None, variables: vec![], scripts: Scripts::default(), postman_extra: Extra::new() };
    store.create_node(None, NodeKind::Collection, name, &NodeContent::Collection(content)).unwrap()
}
fn stored(store: &Store) -> serde_json::Value {
    serde_json::from_str(&store.state_get(SCRIPTS_ALLOWED_KEY).unwrap().unwrap()).unwrap()
}

#[test]
fn nothing_is_allowed_by_default() {
    let t = tree();
    assert_eq!(t.store.state_get(SCRIPTS_ALLOWED_KEY).unwrap(), None);
    assert!(!scripts_allowed(&t.store, &t.col).unwrap());
}

#[test]
fn allow_and_revoke() {
    let mut t = tree();
    set_scripts_allowed(&mut t.store, &t.col, true).unwrap();
    set_scripts_allowed(&mut t.store, &t.col, true).unwrap();
    assert!(scripts_allowed(&t.store, &t.col).unwrap());
    assert_eq!(stored(&t.store), serde_json::json!([t.col]), "an id is listed once");

    set_scripts_allowed(&mut t.store, &t.col, false).unwrap();
    assert!(!scripts_allowed(&t.store, &t.col).unwrap());
    assert_eq!(stored(&t.store), serde_json::json!([]));
}

#[test]
fn unreadable_value_counts_as_empty() {
    for value in ["abc", "{}", "[1]"] {
        let mut t = tree();
        t.store.state_set(SCRIPTS_ALLOWED_KEY, value).unwrap();
        assert!(!scripts_allowed(&t.store, &t.col).unwrap(), "{value}");
        // And the next consent starts a readable list.
        set_scripts_allowed(&mut t.store, &t.col, true).unwrap();
        assert_eq!(stored(&t.store), serde_json::json!([t.col]), "{value}");
    }
}

#[test]
fn forget_deleted_collections_drops_missing_ids() {
    let mut t = tree();
    let other = collection(&mut t.store, "Other");
    set_scripts_allowed(&mut t.store, &t.col, true).unwrap();
    set_scripts_allowed(&mut t.store, &other, true).unwrap();
    t.store.delete_node(&t.col).unwrap();

    forget_deleted_collections(&mut t.store).unwrap();
    assert_eq!(stored(&t.store), serde_json::json!([other]));
}

#[test]
fn collection_of_a_request_a_folder_and_a_collection() {
    let t = tree();
    for node in [&t.req, &t.folder, &t.col] {
        assert_eq!(collection_of(&t.store, node).unwrap(), t.col);
    }
    assert!(matches!(collection_of(&t.store, "missing"), Err(StoreError::NotFound(_))));
}

#[test]
fn load_send_input_decides_the_mode() {
    let mut t = tree();
    let mode = |store: &Store, settings: AppSettings| load_send_input(store, &t.req, None, None, settings).unwrap().scripts;

    assert_eq!(
        mode(&t.store, AppSettings::default()),
        ScriptsMode::Off(ScriptsOff::NotAllowed { collection: "Col".into() })
    );
    set_scripts_allowed(&mut t.store, &t.col, true).unwrap();
    assert_eq!(mode(&t.store, AppSettings::default()), ScriptsMode::Run);
    let disabled = AppSettings { scripts_enabled: false, ..AppSettings::default() };
    assert_eq!(mode(&t.store, disabled), ScriptsMode::Off(ScriptsOff::Disabled));
}
