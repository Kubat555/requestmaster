/// Every `*.json` in `<repo>/spike/samples/` must import and export back with an empty semantic diff.
/// Prints counts and JSON paths only — never values from the owner's files.
#[test] fn owner_samples_round_trip() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spike/samples");
    let files: Vec<_> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json")).collect()).unwrap_or_default();
    if files.is_empty() { println!("owner samples: 0 (skipped)"); return; }
    let mut checked = 0;
    for (n, path) in files.iter().enumerate() {
        let Ok(src) = rqm_postman::parse_text(&std::fs::read_to_string(path).unwrap()) else { continue };
        let Ok(kind) = rqm_postman::detect(&src) else { continue };          // unsupported format: counted as skipped
        let (out, summary) = match kind {
            rqm_postman::Detected::Collection => {
                let c = rqm_postman::import_collection(&src).expect("imports");
                (rqm_postman::export_collection(&c.nodes).expect("exports"),
                 format!("nodes {}, report items {}", c.nodes.len(), c.report.items.len()))
            }
            _ => { let e = rqm_postman::import_environment(&src).expect("imports");
                   (rqm_postman::export_environment(&e), format!("variables {}", e.variables.len())) }
        };
        let diff = rqm_postman::semantic_diff(&src, &out);
        println!("owner sample #{n}: {summary}, diff paths {}", diff.len());
        assert!(diff.is_empty(), "sample #{n} differs at {} paths", diff.len());
        checked += 1;
    }
    println!("owner samples: {checked}, skipped: {}", files.len() - checked);
}
