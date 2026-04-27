//! Gecko-sim content: RON catalog loaders.
//!
//! Public surface: [`load_from_dir`] reads a content root and returns a
//! populated [`ContentBundle`]. The loader does file globbing and parsing;
//! validation runs on the loaded entries before they are collected.

use std::path::Path;

use gecko_sim_core::ContentBundle;

mod loader;

pub mod error;

pub use error::ContentError;

/// Load all RON catalog files under `root` into a [`ContentBundle`].
///
/// Layout expected:
///
/// ```text
/// <root>/
///   object_types/
///     *.ron      // one ObjectType per file
///   accessories/
///     *.ron      // one Accessory per file
/// ```
///
/// Each subdirectory is optional; a missing directory contributes zero
/// entries. Files are visited in lexicographic path order for
/// deterministic load behaviour; only lowercase `.ron` extensions match.
/// Validation (unique IDs, predicate well-formedness) is wired in by
/// Task 5.
pub fn load_from_dir(root: &Path) -> Result<ContentBundle, ContentError> {
    let object_types = loader::load_object_types(root)?;
    let accessories = loader::load_accessories(root)?;

    let mut bundle = ContentBundle::default();
    for (_, ot) in object_types {
        bundle.object_types.insert(ot.id, ot);
    }
    for (_, acc) in accessories {
        bundle.accessories.insert(acc.id, acc);
    }
    Ok(bundle)
}
