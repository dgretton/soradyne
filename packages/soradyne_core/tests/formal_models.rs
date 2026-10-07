//! Runs retained TLA+ mechanism sketches when their tooling is available.
//!
//! This is not verification of the complete shared-flow architecture or network
//! partition safety. See docs/models/README.md for scope and assumptions.
//! SORADYNE_REQUIRE_FORMAL=1 turns unavailable tooling from a loud skip into failure.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    // packages/soradyne_core -> repo root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

#[test]
fn formal_models_hold() {
    let script = repo_root().join("docs/models/check.sh");
    assert!(script.exists(), "missing {}", script.display());

    let status = Command::new("bash")
        .arg(&script)
        .status()
        .expect("failed to spawn docs/models/check.sh");

    match status.code() {
        Some(0) => {}
        Some(2) => {
            let msg =
                "formal models SKIPPED: Java or network unavailable (docs/models/check.sh exit 2)";
            if std::env::var("SORADYNE_REQUIRE_FORMAL").is_ok() {
                panic!("{msg}, and SORADYNE_REQUIRE_FORMAL is set");
            }
            eprintln!("\n*** {msg} ***\n");
        }
        other => panic!(
            "formal models FAILED (docs/models/check.sh exit {:?}); read the counterexample above",
            other
        ),
    }
}
