//! A big synthetic collection for the measurements; nothing in it comes from a real file.

use serde_json::{json, Value};

/// A Postman v2.1 collection of `folders` folders with `requests_per_folder` requests in each: a `GET` with
/// two query parameters on every other place, a `POST` with a JSON body on the places between them.
pub fn big_collection(folders: usize, requests_per_folder: usize) -> Value {
    let folder = |f: usize| {
        let requests: Vec<Value> = (0..requests_per_folder).map(|r| request(f, r)).collect();
        json!({ "name": format!("Folder {f:03}"), "item": requests })
    };
    json!({
        "info": {
            "name": format!("Synthetic {}", folders * requests_per_folder),
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "variable": [{ "key": "host", "value": "http://127.0.0.1:8080" }],
        "item": (0..folders).map(folder).collect::<Vec<Value>>()
    })
}

/// The request number `r` of the folder number `f`.
fn request(f: usize, r: usize) -> Value {
    let url = format!("{{{{host}}}}/folder-{f}/items/{r}");
    let headers = json!([{ "key": "Accept", "value": "application/json" }]);
    if r.is_multiple_of(2) {
        json!({
            "name": format!("Get item {f:03}-{r:03}"),
            "request": {
                "method": "GET",
                "header": headers,
                "url": {
                    "raw": format!("{url}?page=1&size=50"),
                    "query": [{ "key": "page", "value": "1" }, { "key": "size", "value": "50" }]
                }
            }
        })
    } else {
        json!({
            "name": format!("Create item {f:03}-{r:03}"),
            "request": {
                "method": "POST",
                "header": headers,
                "body": {
                    "mode": "raw",
                    "raw": format!("{{\"folder\": {f}, \"item\": {r}, \"name\": \"item {f}-{r}\"}}"),
                    "options": { "raw": { "language": "json" } }
                },
                "url": { "raw": url }
            }
        })
    }
}
