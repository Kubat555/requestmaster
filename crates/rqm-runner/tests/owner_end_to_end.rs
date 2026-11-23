/// Owner collections: import → store → read back → export with an empty diff; every request can be prepared.
/// Prints counts only — never names, URLs or values from the owner's files. Nothing is sent to the network.
#[test] fn owner_collections_survive_the_store_and_every_request_prepares() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spike/samples");
    let files: Vec<_> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json")).collect()).unwrap_or_default();
    if files.is_empty() { println!("owner samples: 0 (skipped)"); return; }
    let mut collections = 0;
    for (n, path) in files.iter().enumerate() {
        let Ok(src) = rqm_postman::parse_text(&std::fs::read_to_string(path).unwrap()) else { continue };
        if !matches!(rqm_postman::detect(&src), Ok(rqm_postman::Detected::Collection)) { continue; }
        collections += 1;
        let imported = rqm_postman::import_collection(&src).unwrap();
        let mut store = rqm_store::Store::open_in_memory().unwrap();
        store.insert_nodes(&imported.nodes).unwrap();
        let root = imported.nodes[0].id.clone();
        let stored = store.subtree(&root).unwrap();
        assert_eq!(stored.len(), imported.nodes.len(), "sample #{n}: node count");
        let diff = rqm_postman::semantic_diff(&src, &rqm_postman::export_collection(&stored).unwrap());
        assert!(diff.is_empty(), "sample #{n}: {} paths differ after the store", diff.len());

        let (mut prepared, mut blocked, mut with_script_warning, mut unresolved) = (0, 0, 0, 0);
        for node in stored.iter().filter(|x| x.kind == rqm_model::NodeKind::Request) {
            let input = rqm_runner::load_send_input(&store, &node.id, None, None, rqm_runner::AppSettings::default()).unwrap();
            let has_scripts = [rqm_runner::ScriptPhase::PreRequest, rqm_runner::ScriptPhase::Test].into_iter()
                .any(|phase| !rqm_runner::script_steps(&input, phase).is_empty());
            match rqm_runner::prepare(&input, &mut |_| None) {
                Ok(p) => {
                    prepared += 1;
                    if has_scripts { with_script_warning += 1; }
                    if p.warnings.iter().any(|w| matches!(w, rqm_runner::SendWarning::UnresolvedVariables { .. })) { unresolved += 1; }
                }
                Err(_) => blocked += 1,
            }
        }
        println!("owner sample #{n}: requests prepared {prepared}, blocked {blocked}, with script warning {with_script_warning}, with unresolved variables {unresolved}, report items {}", imported.report.items.len());
        assert_eq!(prepared + blocked, stored.iter().filter(|x| x.kind == rqm_model::NodeKind::Request).count());
    }
    println!("owner collections: {collections}");
}

/// The owner collections among the samples, parsed, in the order of the folder; no folder or no file — none.
fn owner_collections() -> Vec<serde_json::Value> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spike/samples");
    let mut files: Vec<_> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json")).collect()).unwrap_or_default();
    files.sort();
    let parsed = files.iter().filter_map(|path| rqm_postman::parse_text(&std::fs::read_to_string(path).ok()?).ok());
    parsed.filter(|src| matches!(rqm_postman::detect(src), Ok(rqm_postman::Detected::Collection))).collect()
}

/// Every script of the owner collections compiles in the engine. Nothing of a script runs: the engine wraps
/// the whole text into one function, and `return` on the first line leaves it at once.
/// Prints counts only; a failure names the number of the sample and of the script — no names, text or values.
#[test] fn owner_scripts_compile_in_the_engine() {
    use rqm_model::NodeContent;
    let samples = owner_collections();
    if samples.is_empty() { println!("owner samples: 0 (skipped)"); return; }
    let (mut blocks, mut compiled, mut with_unsupported) = (0, 0, 0);
    // (number of the sample, number of the script in it).
    let mut failed: Vec<(usize, usize)> = Vec::new();
    for (n, src) in samples.iter().enumerate() {
        let imported = rqm_postman::import_collection(src).unwrap_or_else(|_| panic!("sample #{n}: import failed"));
        let scripts = imported.nodes.iter().flat_map(|node| {
            let scripts = match &node.content {
                NodeContent::Collection(content) => &content.scripts,
                NodeContent::Folder(content) => &content.scripts,
                NodeContent::Request(content) => &content.scripts,
            };
            [&scripts.pre_request, &scripts.test]
        });
        for (ordinal, code) in scripts.filter(|code| !code.trim().is_empty()).enumerate() {
            blocks += 1;
            let job = rqm_script::Job::new(rqm_script::Phase::Test, vec![format!("return;\n{code}")]);
            if rqm_script::run(job, &tokio_util::sync::CancellationToken::new()).errors.is_empty() {
                compiled += 1;
            } else {
                failed.push((n, ordinal));
            }
            if !rqm_script::unsupported_api(code).is_empty() { with_unsupported += 1; }
        }
    }
    println!("owner scripts: blocks {blocks}, compiled {compiled}, failed {}, with unsupported api {with_unsupported}", failed.len());
    assert!(failed.is_empty(), "scripts that do not compile (sample #, script #): {failed:?}");
}

/// A variable a script changed is the only thing that changes in the export of an owner collection.
/// Prints counts only; a failure names the number of the sample and counts.
#[test] fn owner_collection_export_survives_a_script_change() {
    use rqm_model::NodeContent;
    use rqm_runner::{apply_variable_changes, StoredScope, VariableChange, VariableOp};
    let samples = owner_collections();
    if samples.is_empty() { println!("owner samples: 0 (skipped)"); return; }
    let (mut outside_total, mut rows_total) = (0, 0);
    for (n, src) in samples.iter().enumerate() {
        let imported = rqm_postman::import_collection(src).unwrap_or_else(|_| panic!("sample #{n}: import failed"));
        let mut store = rqm_store::Store::open_in_memory().unwrap();
        store.insert_nodes(&imported.nodes).unwrap();
        let root = imported.nodes[0].id.clone();
        let export = |store: &rqm_store::Store| rqm_postman::export_collection(&store.subtree(&root).unwrap()).unwrap();
        let before = export(&store);

        let NodeContent::Collection(content) = &imported.nodes[0].content else { panic!("sample #{n}: no collection") };
        let first_enabled = content.variables.iter().find(|variable| variable.enabled);
        let key = first_enabled.map_or_else(|| "rqm_probe".to_string(), |variable| variable.key.clone());
        let change = VariableChange { scope: StoredScope::Collection, op: VariableOp::Set { key, value: "rqm-test".into() } };
        apply_variable_changes(&mut store, &root, None, &[change]).unwrap();
        let after = export(&store);

        let in_variable = |path: &&String| *path == "$.variable" || path.starts_with("$.variable[");
        let outside = rqm_postman::semantic_diff(&before, &after).iter().filter(|path| !in_variable(path)).count();
        let rows = |json: &serde_json::Value| json.get("variable").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let (rows_before, rows_after) = (rows(&before), rows(&after));
        let changed = (0..rows_before.len().max(rows_after.len())).filter(|&i| rows_before.get(i) != rows_after.get(i)).count();
        assert_eq!((outside, changed), (0, 1), "sample #{n}: (differences outside variable, changed variable rows)");
        outside_total += outside;
        rows_total += changed;
    }
    println!("owner export after a script change: collections {}, differences outside variable {outside_total}, changed variable rows {rows_total}", samples.len());
    assert_eq!((outside_total, rows_total), (0, samples.len()));
}
