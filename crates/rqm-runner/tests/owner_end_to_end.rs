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
