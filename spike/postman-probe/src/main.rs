//! Walks a samples directory and prints an anonymised statistics report (counts and key names only).
use postman_probe::*;
use serde_json::Value;
use std::collections::BTreeMap;

fn bump(m: &mut BTreeMap<String, usize>, k: impl Into<String>) {
    *m.entry(k.into()).or_insert(0) += 1;
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "../samples".into());
    let (mut collections, mut folders, mut requests) = (0, 0, 0);
    let mut schemas = BTreeMap::new();
    let mut auth_types = BTreeMap::new();
    let mut body_modes = BTreeMap::new();
    let mut ppb_keys = BTreeMap::new();
    let mut usage: BTreeMap<String, usize> = BTreeMap::new();
    let mut zips: Vec<String> = Vec::new();
    let mut skipped = 0;

    for e in walkdir::WalkDir::new(&dir).into_iter().flatten() {
        let p = e.path();
        match p.extension().and_then(|x| x.to_str()) {
            Some("zip") => zips.push(p.display().to_string()),
            Some("json") => {
                let src = std::fs::read_to_string(p).unwrap();
                let is_collection = serde_json::from_str::<Value>(&src)
                    .map(|v| v["info"]["schema"].is_string())
                    .unwrap_or(false);
                if !is_collection {
                    skipped += 1;
                    continue;
                }
                let c = match parse_collection(&src) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("PARSE ERROR in a collection: {e}");
                        continue;
                    }
                };
                collections += 1;
                folders += count_folders(&c);
                requests += count_requests(&c);
                bump(&mut schemas, c.info.schema.clone().unwrap_or_default());
                if let Some(a) = &c.auth {
                    bump(&mut auth_types, a.kind.clone());
                }
                if let Some(v) = c.extra.get("protocolProfileBehavior").and_then(|v| v.as_object()) {
                    v.keys().for_each(|k| bump(&mut ppb_keys, k.clone()));
                }
                walk_items(&c.item, &mut |i| match i {
                    Item::Folder(f) => {
                        if let Some(a) = &f.auth {
                            bump(&mut auth_types, a.kind.clone());
                        }
                        if let Some(v) = f.extra.get("protocolProfileBehavior").and_then(|v| v.as_object()) {
                            v.keys().for_each(|k| bump(&mut ppb_keys, k.clone()));
                        }
                    }
                    Item::Request(r) => {
                        if let Some(a) = &r.auth {
                            bump(&mut auth_types, a.kind.clone());
                        }
                        if let Some(v) = r.extra.get("protocolProfileBehavior").and_then(|v| v.as_object()) {
                            v.keys().for_each(|k| bump(&mut ppb_keys, k.clone()));
                        }
                        if let Request::Obj(o) = &r.request {
                            if let Some(a) = &o.auth {
                                bump(&mut auth_types, a.kind.clone());
                            }
                            if let Some(b) = &o.body {
                                bump(&mut body_modes, b.mode.clone().unwrap_or_else(|| "(none)".into()));
                            }
                        }
                    }
                });
                for (k, n) in script_api_usage(&c) {
                    *usage.entry(k).or_insert(0) += n;
                }
            }
            _ => {}
        }
    }

    println!("collections: {collections}  folders: {folders}  requests: {requests}  (other json skipped: {skipped})");
    println!("info.schema: {schemas:?}");
    println!("auth types: {auth_types:?}");
    println!("body modes: {body_modes:?}");
    println!("protocolProfileBehavior keys: {ppb_keys:?}");
    println!("script api usage:");
    for (k, n) in &usage {
        println!("  {k}: {n}");
    }
    if zips.is_empty() {
        println!("zip dumps: none found");
    } else {
        // No zip crate in this probe: only the archive file names are listed.
        println!("zip dumps (contents not inspected): {zips:?}");
    }
}
