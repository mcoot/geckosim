//! Locks the contract that the workspace-root `content/` directory parses
//! cleanly with the schema currently in `core`. If a future schema change
//! breaks the seed, this test fires before the host smoke does.

use std::path::PathBuf;

use gecko_sim_content::load_from_dir;
use gecko_sim_core::ids::{AccessoryId, ObjectTypeId};

fn workspace_content_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR for this test = <workspace>/crates/content
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("content")
}

#[test]
fn seed_content_loads() {
    let root = workspace_content_dir();
    let bundle = load_from_dir(&root)
        .unwrap_or_else(|e| panic!("loading {}: {e}", root.display()));

    assert_eq!(bundle.object_types.len(), 2, "expected 2 object types");
    assert!(bundle.object_types.contains_key(&ObjectTypeId::new(1)));
    assert!(bundle.object_types.contains_key(&ObjectTypeId::new(2)));

    assert_eq!(bundle.accessories.len(), 2, "expected 2 accessories");
    assert!(bundle.accessories.contains_key(&AccessoryId::new(1)));
    assert!(bundle.accessories.contains_key(&AccessoryId::new(2)));
}
