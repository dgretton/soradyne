//! Runs the formal models in `docs/models/` (Alloy invariants, TLA+ lease) as part
//! of `cargo test`, when Java is available.
//!
//! The models are the executable form of the consolidated model's invariants
//! (`docs/20260906_fable_soradyne_cascade.md` §3.2). They must pass on the
//! development machine. On a machine without Java the test prints a loud skip and
//! passes, so that `cargo test` stays usable everywhere; set
//! `SORADYNE_REQUIRE_FORMAL=1` to turn the skip into a failure (do this in CI).
//!
//! Run directly with `docs/models/check.sh`; see `docs/models/README.md`.

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
            let msg = "formal models SKIPPED: Java or network unavailable (docs/models/check.sh exit 2)";
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
