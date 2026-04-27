//! Shared helpers for `crates/core/tests/*.rs` integration tests.

use std::path::PathBuf;

use gecko_sim_core::ContentBundle;

/// Resolve the workspace-root `content/` directory and load the seed
/// catalog. Equivalent to `gecko-sim-content::load_from_dir(<workspace>/content)`.
pub fn seed_content_bundle() -> ContentBundle {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("content");
    gecko_sim_content::load_from_dir(&root)
        .unwrap_or_else(|e| panic!("loading seed content from {}: {e}", root.display()))
}
