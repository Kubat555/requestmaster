//! The prelude: the built files match their sources, the prelude loads into every script and keeps the
//! primitives of the host out of reach.

use rqm_script::{run, Job, Phase};
use std::fs;
use std::path::Path;
use tokio_util::sync::CancellationToken;

/// FNV-1a 64, the same sum `prelude/build.mjs` writes into the first line of a built file.
struct Fnv(u64);

impl Fnv {
    fn feed(&mut self, bytes: impl IntoIterator<Item = u8>) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }
}

#[test]
fn built_files_match_their_sources() {
    let prelude = Path::new(env!("CARGO_MANIFEST_DIR")).join("prelude");
    let mut sources: Vec<String> = fs::read_dir(prelude.join("src"))
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_type().unwrap().is_file())
        .map(|entry| format!("src/{}", entry.file_name().to_str().unwrap()))
        .collect();
    sources.sort();
    let mut names = vec![
        "build.mjs".to_string(),
        "package.json".to_string(),
        "package-lock.json".to_string(),
    ];
    names.append(&mut sources);

    let mut sum = Fnv(0xcbf2_9ce4_8422_2325);
    for name in &names {
        sum.feed(name.bytes());
        sum.feed([0x0a]);
        // Without 0x0D: a checkout with Windows line ends has the same sum.
        let content = fs::read(prelude.join(name)).unwrap();
        sum.feed(content.into_iter().filter(|byte| *byte != 0x0d));
        sum.feed([0x0a]);
    }

    // Every file in `built/` carries the sum of the same sources.
    let mut checked = 0;
    for entry in fs::read_dir(prelude.join("built")).unwrap() {
        let path = entry.unwrap().path();
        let built = fs::read_to_string(&path).unwrap();
        assert_eq!(
            built.lines().next().unwrap(),
            format!("// rqm-prelude-hash: {:016x}", sum.0),
            "{}: the sources of the prelude changed: run `npm run build` in crates/rqm-script/prelude",
            path.display()
        );
        checked += 1;
    }
    assert!(checked >= 2, "built/ holds the prelude and chai");
}

#[test]
fn prelude_loads_into_every_script() {
    let scripts = vec!["console.log(pm.info.eventName)".to_string(); 30];
    let report = run(Job::new(Phase::Test, scripts), &CancellationToken::new());
    assert_eq!(report.errors, vec![]);
    assert_eq!(report.console.len(), 30);
    assert!(report.console.iter().all(|line| line.text == "test"));
}

#[test]
fn host_primitives_are_out_of_reach() {
    let script = "if (typeof __host !== 'undefined') throw new Error('host')".to_string();
    let report = run(
        Job::new(Phase::Test, vec![script]),
        &CancellationToken::new(),
    );
    assert_eq!(report.errors, vec![]);
}
