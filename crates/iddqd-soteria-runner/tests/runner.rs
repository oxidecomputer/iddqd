//! Runs each iddqd Soteria proof in its own process so `cargo nextest run` can
//! schedule them across cores. Soteria executes entrypoints serially and largely
//! single-threaded, so we use nextest to get parallelism.
//!
//! The ULLBC is compiled once by the nextest setup script; each test here
//! reuses it with `--no-compile`.
//!
//! These tests shell out to the Soteria toolchain, so they are `#[ignore]`d by
//! default and only run via `just soteria`.

use std::{path::PathBuf, process::Command};

fn workspace_root() -> PathBuf {
    std::env::var_os("NEXTEST_WORKSPACE_ROOT").map(PathBuf::from).expect(
        "NEXTEST_WORKSPACE_ROOT should be set by nextest; run the Soteria \
         proofs via `just soteria`",
    )
}

/// Shells out to one Soteria entrypoint, reusing the pre-compiled ULLBC.
///
/// `name` is matched as a trailing path segment (`::name$`), which is robust to
/// the module nesting in `tests/soteria/`.
fn run(mode_args: &[&str], name: &str) {
    let root = workspace_root();
    let status = Command::new(root.join("scripts/soteria-rust"))
        .arg("exec")
        .arg(root.join("crates/iddqd"))
        .args(mode_args)
        .args(["--no-compile", "--ignore-leaks", "--no-color", "--filter"])
        .arg(format!("::{name}$"))
        .status()
        .expect("spawned scripts/soteria-rust");
    assert!(status.success(), "Soteria proof `{name}` failed");
}

/// A proof defined in `crates/iddqd/src/proofs.rs`.
macro_rules! lib_proof {
    ($name:ident) => {
        #[test]
        #[ignore = "Soteria proof; run via `just soteria`"]
        fn $name() {
            run(&[], stringify!($name));
        }
    };
}

// Keep in sync with the `#[soteria::test]` entrypoints in iddqd/src/proofs.rs.
lib_proof!(item_set_insert_assigns_dense_indexes);
lib_proof!(item_set_remove_then_insert_reuses_freed_slot);
