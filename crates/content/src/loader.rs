//! File-globbing RON loaders for the `object_types/` and `accessories/`
//! subdirectories. Loaders return owned `Vec<(PathBuf, T)>` so each entry's
//! source path can flow into validation error messages.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gecko_sim_core::agent::Accessory;
use gecko_sim_core::object::ObjectType;
use serde::de::DeserializeOwned;

use crate::error::ContentError;

const OBJECT_TYPES_SUBDIR: &str = "object_types";
const ACCESSORIES_SUBDIR: &str = "accessories";
const RON_EXT: &str = "ron";

/// Load every `*.ron` file in `<root>/object_types/`. Files are sorted by
/// full path for deterministic load order; for the v0 single-level layout
/// this matches filename order. Subdirectories are ignored. Missing
/// `object_types/` directory yields an empty vec. Extension match is
/// case-sensitive — only lowercase `.ron` files are loaded.
pub(crate) fn load_object_types(
    root: &Path,
) -> Result<Vec<(PathBuf, ObjectType)>, ContentError> {
    load_subdir::<ObjectType>(&root.join(OBJECT_TYPES_SUBDIR))
}

/// Load every `*.ron` file in `<root>/accessories/`. Same semantics as
/// `load_object_types`.
pub(crate) fn load_accessories(
    root: &Path,
) -> Result<Vec<(PathBuf, Accessory)>, ContentError> {
    load_subdir::<Accessory>(&root.join(ACCESSORIES_SUBDIR))
}

fn load_subdir<T: DeserializeOwned>(dir: &Path) -> Result<Vec<(PathBuf, T)>, ContentError> {
    let entries = match fs::read_dir(dir) {
        Ok(it) => it,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(ContentError::Io {
                path: dir.to_path_buf(),
                source: e,
            });
        }
    };

    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| ContentError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| ContentError::Io {
            path: path.clone(),
            source: e,
        })?;
        if !file_type.is_file() {
            continue;
        }
        if path.extension().and_then(OsStr::to_str) != Some(RON_EXT) {
            continue;
        }
        paths.push(path);
    }
    paths.sort();

    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let contents = fs::read_to_string(&path).map_err(|e| ContentError::Io {
            path: path.clone(),
            source: e,
        })?;
        let value: T = ron::from_str(&contents).map_err(|e| ContentError::Parse {
            path: path.clone(),
            source: e,
        })?;
        out.push((path, value));
    }
    Ok(out)
}
