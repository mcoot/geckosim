# RON content loading v0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the `gecko-sim-content` crate's RON loaders, populate `ContentBundle` with object types and accessories from disk, insert them as `bevy_ecs` resources in `Sim::new`, and wire the host to load from `GECKOSIM_CONTENT_DIR` (default `<workspace>/content`).

**Architecture:** Loaders glob one-RON-file-per-entry under `content/object_types/` and `content/accessories/`, parse via `ron`, validate (unique IDs, predicate state-key references, score-template sanity), and return a `ContentBundle` holding two `HashMap`s. `Sim::new` decomposes the bundle into split `ObjectCatalog` and `AccessoryCatalog` resources for future systems to query via `Res<…>`. Host fail-fasts on load errors; the WS smoke test continues to use `ContentBundle::default()` unchanged.

**Tech Stack:** Rust 2021, `ron 0.10`, `serde`, `thiserror`, `bevy_ecs 0.16`, `tempfile` (dev-only), `anyhow` (host bridging).

**Reference:** Spec at [`docs/superpowers/specs/2026-04-27-ron-content-loading-design.md`](../specs/2026-04-27-ron-content-loading-design.md). ADR 0011 (schema) and ADR 0012 (architecture) are the source-of-truth ADRs.

**VCS note:** This repo uses `jj` (jujutsu), colocated with git. **Never use raw `git`.** Each task starts with `jj new -m "<task title>"` to create a fresh atomic commit; jj automatically snapshots edits into the current `@` as you work.

---

## File Structure

**New files:**
- `crates/content/src/error.rs` — `ContentError` enum (Task 3)
- `crates/content/src/loader.rs` — file-globbing loaders (Task 4)
- `crates/content/src/validate.rs` — validators (Task 5)
- `crates/content/tests/loader_smoke.rs` — happy-path tempdir round-trip (Task 4)
- `crates/content/tests/validation.rs` — one test per `ContentError` variant (Task 5)
- `crates/content/tests/seed_loads.rs` — load the real `content/` dir (Task 6)
- `crates/core/tests/catalogs.rs` — `Resource` + `Sim::new` insertion test (Tasks 1 & 2)
- `content/object_types/fridge.ron` (Task 6)
- `content/object_types/chair.ron` (Task 6)
- `content/accessories/sunglasses.ron` (Task 6)
- `content/accessories/bowtie.ron` (Task 6)

**Modified files:**
- `crates/core/src/agent/mod.rs` — adds `Accessory`, `AccessorySlot`, `AccessoryCatalog` (Task 1)
- `crates/core/src/object/mod.rs` — adds `ObjectCatalog` (Task 1)
- `crates/core/src/sim.rs` — fattens `ContentBundle`; `Sim::new` inserts resources (Task 2)
- `crates/core/src/lib.rs` — re-exports new types (Task 1)
- `crates/content/src/lib.rs` — replace placeholder with module decls + `load_from_dir` (Tasks 3, 4, 5)
- `crates/content/Cargo.toml` — add `tempfile` dev-dep (Task 4)
- `crates/host/src/config.rs` — `resolve_content_dir`/`content_dir` helpers (Task 7)
- `crates/host/src/main.rs` — call loader, thread bundle into `Sim::new` (Task 7)
- `crates/host/Cargo.toml` — add `gecko-sim-content` dep (Task 7)
- `Cargo.toml` (workspace) — add `tempfile` to `[workspace.dependencies]` (Task 4)
- `content/README.md` — refresh from "later" to layout pointer (Task 6)

---

## Task 1: `Accessory` schema + split catalog `Resource` types

**Files:**
- Modify: `crates/core/src/agent/mod.rs`
- Modify: `crates/core/src/object/mod.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/catalogs.rs`

- [ ] **Step 1.1: Start the task commit**

```bash
jj new -m "RON content: Accessory, AccessorySlot, ObjectCatalog, AccessoryCatalog types"
```

- [ ] **Step 1.2: Write the failing integration test**

Create `crates/core/tests/catalogs.rs`:

```rust
//! Smoke test: the new catalog resources derive `Resource` and can be
//! inserted into a `bevy_ecs::World`.

use std::collections::HashMap;

use bevy_ecs::world::World;
use gecko_sim_core::agent::{Accessory, AccessoryCatalog, AccessorySlot};
use gecko_sim_core::ids::{AccessoryId, ObjectTypeId};
use gecko_sim_core::object::{MeshId, ObjectCatalog, ObjectType};

#[test]
fn object_catalog_resource_inserts() {
    let mut world = World::new();
    let mut by_id = HashMap::new();
    by_id.insert(
        ObjectTypeId::new(1),
        ObjectType {
            id: ObjectTypeId::new(1),
            display_name: "Fridge".into(),
            mesh_id: MeshId(1),
            default_state: HashMap::new(),
            advertisements: vec![],
        },
    );
    world.insert_resource(ObjectCatalog { by_id });
    let res = world
        .get_resource::<ObjectCatalog>()
        .expect("ObjectCatalog inserted");
    assert_eq!(res.by_id.len(), 1);
}

#[test]
fn accessory_catalog_resource_inserts() {
    let mut world = World::new();
    let mut by_id = HashMap::new();
    by_id.insert(
        AccessoryId::new(1),
        Accessory {
            id: AccessoryId::new(1),
            display_name: "Sunglasses".into(),
            mesh_id: MeshId(101),
            slot: AccessorySlot::Head,
        },
    );
    world.insert_resource(AccessoryCatalog { by_id });
    let res = world
        .get_resource::<AccessoryCatalog>()
        .expect("AccessoryCatalog inserted");
    assert_eq!(res.by_id.len(), 1);
}

#[test]
fn accessory_slot_round_trips_via_ron() {
    let v = AccessorySlot::Neck;
    let s = ron::to_string(&v).expect("serialize");
    let back: AccessorySlot = ron::from_str(&s).expect("deserialize");
    assert_eq!(v, back);
}
```

- [ ] **Step 1.3: Run the failing test**

```bash
cargo test -p gecko-sim-core --test catalogs
```

Expected: compile errors on `Accessory`, `AccessorySlot`, `AccessoryCatalog`, `ObjectCatalog` ("not found in module").

- [ ] **Step 1.4: Add `Accessory`, `AccessorySlot`, `AccessoryCatalog` in `crates/core/src/agent/mod.rs`**

Append after the existing `Appearance` struct (around line 77 — after the `accessories: Vec<AccessoryId>` field's closing brace):

```rust
// ---------------------------------------------------------------------------
// Accessory catalog (aesthetic-only at v0 per ADR 0011)
// ---------------------------------------------------------------------------

/// Where on a gecko an `Accessory` attaches. Mirrors the four-slot cap on
/// `Appearance.accessories`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessorySlot {
    Head,
    Neck,
    Body,
    Tail,
}

/// Static accessory catalog entry. Aesthetic-only at v0 — no scoring
/// effects, no economic value, no theft. The `mesh_id` is a renderer hint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Accessory {
    pub id: AccessoryId,
    pub display_name: String,
    pub mesh_id: crate::object::MeshId,
    pub slot: AccessorySlot,
}

/// `bevy_ecs` resource holding the loaded accessory catalog. Keyed by
/// `AccessoryId`. Inserted by `Sim::new`.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone, Default)]
pub struct AccessoryCatalog {
    pub by_id: std::collections::HashMap<AccessoryId, Accessory>,
}
```

- [ ] **Step 1.5: Add `ObjectCatalog` in `crates/core/src/object/mod.rs`**

Append at the end of the file:

```rust
// ---------------------------------------------------------------------------
// Object catalog resource
// ---------------------------------------------------------------------------

/// `bevy_ecs` resource holding the loaded object-type catalog. Keyed by
/// `ObjectTypeId`. Inserted by `Sim::new`.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone, Default)]
pub struct ObjectCatalog {
    pub by_id: std::collections::HashMap<ObjectTypeId, ObjectType>,
}
```

- [ ] **Step 1.6: Re-export the new types in `crates/core/src/lib.rs`**

Find the existing `pub use` block (around lines 18-27). Add two new re-export lines so the public API matches the test imports:

```rust
pub use agent::{Accessory, AccessoryCatalog, AccessorySlot};
pub use object::ObjectCatalog;
```

Place them after the `pub use ids::{…};` block, alongside other module re-exports.

- [ ] **Step 1.7: Run the test to verify it passes**

```bash
cargo test -p gecko-sim-core --test catalogs
```

Expected: `test result: ok. 3 passed`.

- [ ] **Step 1.8: Verify the workspace still builds clean**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all clean, all tests pass. The existing `crates/core/tests/{smoke,needs_decay,determinism}.rs`, `crates/host/tests/ws_smoke.rs`, and `crates/protocol/tests/roundtrip.rs` continue to pass unchanged because no public API has changed.

- [ ] **Step 1.9: Confirm the commit is atomic**

```bash
jj st
jj diff --stat
```

Expected: working copy `@` is the new commit "RON content: Accessory, AccessorySlot, ObjectCatalog, AccessoryCatalog types"; modifications confined to `crates/core/src/{agent/mod.rs,object/mod.rs,lib.rs}` and the new `crates/core/tests/catalogs.rs`.

---

## Task 2: `ContentBundle` holds populated catalogs; `Sim::new` inserts resources

**Files:**
- Modify: `crates/core/src/sim.rs`
- Modify: `crates/core/tests/catalogs.rs` (extend with one new test)

- [ ] **Step 2.1: Start the task commit**

```bash
jj new -m "RON content: ContentBundle holds catalogs; Sim::new inserts resources"
```

- [ ] **Step 2.2: Write the failing test**

Append to `crates/core/tests/catalogs.rs`:

```rust
#[test]
fn sim_new_inserts_object_and_accessory_catalogs_from_bundle() {
    use gecko_sim_core::{ContentBundle, Sim};

    let mut object_types = HashMap::new();
    object_types.insert(
        ObjectTypeId::new(7),
        ObjectType {
            id: ObjectTypeId::new(7),
            display_name: "Chair".into(),
            mesh_id: MeshId(2),
            default_state: HashMap::new(),
            advertisements: vec![],
        },
    );
    let mut accessories = HashMap::new();
    accessories.insert(
        AccessoryId::new(9),
        Accessory {
            id: AccessoryId::new(9),
            display_name: "Bow tie".into(),
            mesh_id: MeshId(102),
            slot: AccessorySlot::Neck,
        },
    );
    let bundle = ContentBundle {
        object_types,
        accessories,
    };

    let sim = Sim::new(0, bundle);

    // The catalogs are exposed for tests via dedicated accessors.
    assert_eq!(sim.object_catalog().by_id.len(), 1);
    assert!(sim
        .object_catalog()
        .by_id
        .contains_key(&ObjectTypeId::new(7)));
    assert_eq!(sim.accessory_catalog().by_id.len(), 1);
    assert!(sim
        .accessory_catalog()
        .by_id
        .contains_key(&AccessoryId::new(9)));
}

#[test]
fn sim_new_with_default_bundle_has_empty_catalogs() {
    use gecko_sim_core::{ContentBundle, Sim};

    let sim = Sim::new(0, ContentBundle::default());
    assert!(sim.object_catalog().by_id.is_empty());
    assert!(sim.accessory_catalog().by_id.is_empty());
}
```

- [ ] **Step 2.3: Run the failing test**

```bash
cargo test -p gecko-sim-core --test catalogs sim_new
```

Expected: compile errors — `ContentBundle` is a unit struct, `Sim::object_catalog`/`accessory_catalog` don't exist.

- [ ] **Step 2.4: Fatten `ContentBundle` and wire `Sim::new` in `crates/core/src/sim.rs`**

Replace the current contents of `crates/core/src/sim.rs` with:

```rust
//! Live `Sim` driver: wraps a `bevy_ecs::World` and advances tick state.
//!
//! Honours the public API contract from ADR 0012 partially:
//!   - `new`, `tick`, `current_tick`, `snapshot`.
//!   - `delta_since`, `apply_input` deferred to a later pass.

use std::collections::HashMap;

use bevy_ecs::world::World;

use crate::agent::{Accessory, AccessoryCatalog, Identity, Needs};
use crate::ids::{AccessoryId, AgentId, ObjectTypeId};
use crate::object::{ObjectCatalog, ObjectType};
use crate::rng::PrngState;
use crate::snapshot::{AgentSnapshot, Snapshot};

/// Catalog data passed into `Sim::new`. Loaded from RON files by the
/// `gecko-sim-content` crate; populated maps after a real load, empty maps
/// after `ContentBundle::default()`.
#[derive(Debug, Clone, Default)]
pub struct ContentBundle {
    pub object_types: HashMap<ObjectTypeId, ObjectType>,
    pub accessories: HashMap<AccessoryId, Accessory>,
}

/// Per-tick stats returned from `Sim::tick`. Empty placeholder; future
/// per-tick counters (decisions made, interrupts raised, promoted events
/// emitted, …) live here.
#[derive(Debug, Clone, Default)]
pub struct TickReport;

/// The live simulation. Owns its `bevy_ecs::World` and the canonical clock.
pub struct Sim {
    world: World,
    tick: u64,
    // Stored for determinism; needs-decay does not consume randomness, but
    // future systems will. The seed is captured here at construction time.
    #[expect(dead_code, reason = "consumed by first RNG-using system in a later pass")]
    rng: PrngState,
    next_agent_id: u64,
}

impl Sim {
    /// Construct a fresh sim with the given world seed and content bundle.
    /// The bundle is decomposed into split `ObjectCatalog` and
    /// `AccessoryCatalog` resources on the way into the ECS world.
    #[must_use]
    pub fn new(seed: u64, content: ContentBundle) -> Self {
        let mut world = World::new();
        world.insert_resource(ObjectCatalog {
            by_id: content.object_types,
        });
        world.insert_resource(AccessoryCatalog {
            by_id: content.accessories,
        });
        Self {
            world,
            tick: 0,
            rng: PrngState::from_seed(seed),
            next_agent_id: 0,
        }
    }

    /// Advance the simulation by one tick (one sim-minute per ADR 0008).
    pub fn tick(&mut self) -> TickReport {
        crate::systems::needs::decay(&mut self.world);
        self.tick += 1;
        TickReport
    }

    /// Current tick count. Starts at 0; each `tick()` increments by 1.
    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    /// Borrow the loaded object-type catalog. Mirror of the
    /// `Res<ObjectCatalog>` view that systems will use.
    #[must_use]
    pub fn object_catalog(&self) -> &ObjectCatalog {
        self.world
            .get_resource::<ObjectCatalog>()
            .expect("ObjectCatalog resource is inserted in Sim::new")
    }

    /// Borrow the loaded accessory catalog. Mirror of the
    /// `Res<AccessoryCatalog>` view that systems will use.
    #[must_use]
    pub fn accessory_catalog(&self) -> &AccessoryCatalog {
        self.world
            .get_resource::<AccessoryCatalog>()
            .expect("AccessoryCatalog resource is inserted in Sim::new")
    }

    /// Spawn a fresh agent at full needs with a monotonically allocated
    /// `AgentId`.
    ///
    /// **Note:** this is a placeholder for content-driven agent generation.
    /// It will be replaced in a future pass.
    pub fn spawn_test_agent(&mut self, name: &str) -> AgentId {
        let id = AgentId::new(self.next_agent_id);
        self.next_agent_id += 1;
        self.world.spawn((
            Identity {
                id,
                name: name.to_string(),
            },
            Needs::full(),
        ));
        id
    }

    /// Capture the full sim state at the current tick. Agents are sorted
    /// by `AgentId` ascending for determinism.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let mut agents: Vec<AgentSnapshot> = self
            .world
            .iter_entities()
            .filter_map(|entity_ref| {
                let identity = entity_ref.get::<Identity>()?;
                let needs = entity_ref.get::<Needs>()?;
                Some(AgentSnapshot {
                    id: identity.id,
                    name: identity.name.clone(),
                    needs: *needs,
                })
            })
            .collect();
        agents.sort_by_key(|a| a.id);
        Snapshot {
            tick: self.tick,
            agents,
        }
    }
}
```

- [ ] **Step 2.5: Run the test to verify it passes**

```bash
cargo test -p gecko-sim-core --test catalogs
```

Expected: all 5 tests pass.

- [ ] **Step 2.6: Verify the rest of the workspace still builds**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all green. `crates/host/tests/ws_smoke.rs` (which constructs `Sim::new(seed, ContentBundle::default())`) continues to pass — `Default` still produces empty maps, so behaviour is unchanged for that path.

- [ ] **Step 2.7: Confirm commit**

```bash
jj st
```

Expected: `@` is "RON content: ContentBundle holds catalogs; Sim::new inserts resources" with edits in `crates/core/src/sim.rs` and `crates/core/tests/catalogs.rs`.

---

## Task 3: `ContentError` enum in `gecko-sim-content`

**Files:**
- Create: `crates/content/src/error.rs`
- Modify: `crates/content/src/lib.rs`

This task is a pure type addition — no behaviour, no failing test. Verification gate is `cargo build` clean.

- [ ] **Step 3.1: Start the task commit**

```bash
jj new -m "RON content: ContentError variants in gecko-sim-content"
```

- [ ] **Step 3.2: Create `crates/content/src/error.rs`**

```rust
//! Typed errors for the content loader. Authors hit these at startup —
//! messages always include a path so failures are bisectable.

use std::path::PathBuf;

use gecko_sim_core::agent::Need;
use gecko_sim_core::ids::{AccessoryId, AdvertisementId, ObjectTypeId};
use gecko_sim_core::object::StateKey;

#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse RON in {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: ron::error::SpannedError,
    },

    #[error("duplicate ObjectTypeId {id:?} in {first} and {second}",
        first = first.display(), second = second.display())]
    DuplicateObjectTypeId {
        id: ObjectTypeId,
        first: PathBuf,
        second: PathBuf,
    },

    #[error("duplicate AccessoryId {id:?} in {first} and {second}",
        first = first.display(), second = second.display())]
    DuplicateAccessoryId {
        id: AccessoryId,
        first: PathBuf,
        second: PathBuf,
    },

    #[error("duplicate AdvertisementId {ad:?} within ObjectType {object_type:?} in {path}",
        path = path.display())]
    DuplicateAdvertisementId {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        path: PathBuf,
    },

    #[error(
        "advertisement {ad:?} on ObjectType {object_type:?} ({path}) references unknown ObjectState key {key:?}; \
         known keys: {known:?}",
        path = path.display()
    )]
    UnknownObjectStateKey {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        key: StateKey,
        known: Vec<StateKey>,
        path: PathBuf,
    },

    #[error("advertisement {ad:?} on ObjectType {object_type:?} ({path}) repeats need_weight {need:?}",
        path = path.display())]
    DuplicateNeedWeight {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        need: Need,
        path: PathBuf,
    },

    #[error("advertisement {ad:?} on ObjectType {object_type:?} ({path}) has duration_ticks=0",
        path = path.display())]
    ZeroDuration {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        path: PathBuf,
    },
}
```

- [ ] **Step 3.3: Replace the contents of `crates/content/src/lib.rs`**

```rust
//! Gecko-sim content: RON catalog loaders.
//!
//! Public surface is the `load_from_dir` function (added in a later task)
//! and the `ContentError` typed error.

pub mod error;

pub use error::ContentError;
```

- [ ] **Step 3.4: Verify the crate builds**

```bash
cargo build -p gecko-sim-content
cargo clippy -p gecko-sim-content --all-targets -- -D warnings
```

Expected: clean build. The existing `crates/content/tests/smoke.rs` (`dep_chain_resolves`) keeps passing.

```bash
cargo test -p gecko-sim-content
```

Expected: 1 test passes (the existing smoke test).

- [ ] **Step 3.5: Confirm commit**

```bash
jj st
```

Expected: `@` is "RON content: ContentError variants in gecko-sim-content"; edits confined to `crates/content/src/lib.rs` and the new `crates/content/src/error.rs`.

---

## Task 4: File-globbing loaders + `load_from_dir` entry point

**Files:**
- Create: `crates/content/src/loader.rs`
- Modify: `crates/content/src/lib.rs`
- Modify: `crates/content/Cargo.toml`
- Modify: `Cargo.toml` (workspace)
- Create: `crates/content/tests/loader_smoke.rs`

In this task `load_from_dir` is wired up but does **no** validation — that lands in Task 5. The smoke test feeds valid content that will also pass Task 5's validators.

- [ ] **Step 4.1: Start the task commit**

```bash
jj new -m "RON content: file-globbing loaders for object_types/ and accessories/"
```

- [ ] **Step 4.2: Add `tempfile` to the workspace's `[workspace.dependencies]`**

In `Cargo.toml` at the repo root, append after the existing `futures-util = "0.3"` line in the `[workspace.dependencies]` block:

```toml
tempfile = "3"
```

- [ ] **Step 4.3: Add `tempfile` as a dev-dependency in `crates/content/Cargo.toml`**

Append at the end of the file:

```toml
[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 4.4: Write the failing loader smoke test**

Create `crates/content/tests/loader_smoke.rs`:

```rust
//! End-to-end happy-path test: write a tempdir of valid RON files, call
//! `load_from_dir`, assert the bundle has the expected entries.

use std::fs;

use gecko_sim_content::load_from_dir;
use gecko_sim_core::ids::{AccessoryId, ObjectTypeId};

const FRIDGE_RON: &str = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: { "stocked": Bool(true) },
    advertisements: [
        Advertisement(
            id: AdvertisementId(1),
            display_name: "Eat snack",
            preconditions: [
                ObjectState("stocked", Eq, Bool(true)),
                AgentNeed(Hunger, Lt, 0.6),
            ],
            effects: [
                AgentNeedDelta(Hunger, 0.4),
            ],
            duration_ticks: 10,
            interrupt_class: NeedsThresholdOnly,
            score_template: ScoreTemplate(
                need_weights: [(Hunger, 1.0)],
                personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
"#;

const SUNGLASSES_RON: &str = r#"
Accessory(
    id: AccessoryId(1),
    display_name: "Sunglasses",
    mesh_id: MeshId(101),
    slot: Head,
)
"#;

#[test]
fn load_from_dir_happy_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let object_types_dir = tmp.path().join("object_types");
    let accessories_dir = tmp.path().join("accessories");
    fs::create_dir(&object_types_dir).unwrap();
    fs::create_dir(&accessories_dir).unwrap();
    fs::write(object_types_dir.join("fridge.ron"), FRIDGE_RON).unwrap();
    fs::write(accessories_dir.join("sunglasses.ron"), SUNGLASSES_RON).unwrap();

    let bundle = load_from_dir(tmp.path()).expect("load");

    assert_eq!(bundle.object_types.len(), 1);
    assert!(bundle.object_types.contains_key(&ObjectTypeId::new(1)));
    assert_eq!(
        bundle.object_types[&ObjectTypeId::new(1)].display_name,
        "Fridge"
    );

    assert_eq!(bundle.accessories.len(), 1);
    assert!(bundle.accessories.contains_key(&AccessoryId::new(1)));
}

#[test]
fn load_from_dir_with_missing_subdirs_returns_empty_bundle() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bundle = load_from_dir(tmp.path()).expect("load empty");
    assert!(bundle.object_types.is_empty());
    assert!(bundle.accessories.is_empty());
}

#[test]
fn load_from_dir_skips_non_ron_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let object_types_dir = tmp.path().join("object_types");
    fs::create_dir(&object_types_dir).unwrap();
    fs::write(object_types_dir.join("fridge.ron"), FRIDGE_RON).unwrap();
    fs::write(object_types_dir.join("README.md"), "ignore me").unwrap();
    fs::write(object_types_dir.join(".DS_Store"), "junk").unwrap();

    let bundle = load_from_dir(tmp.path()).expect("load");
    assert_eq!(bundle.object_types.len(), 1);
}
```

- [ ] **Step 4.5: Run the failing test**

```bash
cargo test -p gecko-sim-content --test loader_smoke
```

Expected: compile error — `load_from_dir` is not yet exported from `gecko_sim_content`.

- [ ] **Step 4.6: Create `crates/content/src/loader.rs`**

```rust
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
/// filename for deterministic load order. Subdirectories are ignored.
/// Missing `object_types/` directory yields an empty vec.
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
```

- [ ] **Step 4.7: Update `crates/content/src/lib.rs` to expose `load_from_dir`**

Replace the contents with:

```rust
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
/// entries. Files are visited in lexicographic filename order for
/// deterministic load behaviour. Validation (unique IDs, predicate
/// well-formedness) is wired in by Task 5.
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
```

- [ ] **Step 4.8: Run the smoke test to verify it passes**

```bash
cargo test -p gecko-sim-content --test loader_smoke
```

Expected: 3 tests pass.

- [ ] **Step 4.9: Verify the workspace stays clean**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all green.

- [ ] **Step 4.10: Confirm commit**

```bash
jj st
```

Expected: `@` is "RON content: file-globbing loaders for object_types/ and accessories/"; edits in `Cargo.toml` (workspace), `crates/content/Cargo.toml`, `crates/content/src/{lib,loader}.rs`, and `crates/content/tests/loader_smoke.rs`.

---

## Task 5: Validators + integration into `load_from_dir`

**Files:**
- Create: `crates/content/src/validate.rs`
- Modify: `crates/content/src/lib.rs`
- Create: `crates/content/tests/validation.rs`

- [ ] **Step 5.1: Start the task commit**

```bash
jj new -m "RON content: validators for unique IDs, state-key references, score-template sanity"
```

- [ ] **Step 5.2: Write the failing validation tests**

Create `crates/content/tests/validation.rs`:

```rust
//! One test per `ContentError` validation variant. Each test writes a
//! tempdir that triggers the variant, calls `load_from_dir`, and matches
//! the resulting `Err` by variant.

use std::fs;
use std::path::Path;

use gecko_sim_content::{load_from_dir, ContentError};

fn write_object_type(root: &Path, name: &str, contents: &str) {
    let dir = root.join("object_types");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(name), contents).unwrap();
}

fn write_accessory(root: &Path, name: &str, contents: &str) {
    let dir = root.join("accessories");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(name), contents).unwrap();
}

const FRIDGE_OK: &str = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: { "stocked": Bool(true) },
    advertisements: [
        Advertisement(
            id: AdvertisementId(1),
            display_name: "Eat snack",
            preconditions: [],
            effects: [AgentNeedDelta(Hunger, 0.4)],
            duration_ticks: 10,
            interrupt_class: NeedsThresholdOnly,
            score_template: ScoreTemplate(
                need_weights: [(Hunger, 1.0)],
                personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
"#;

#[test]
fn parse_error_on_malformed_ron() {
    let tmp = tempfile::tempdir().unwrap();
    write_object_type(tmp.path(), "broken.ron", "this is not RON");
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::Parse { .. }),
        "got {err:?}"
    );
}

#[test]
fn duplicate_object_type_id_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    write_object_type(tmp.path(), "fridge.ron", FRIDGE_OK);
    write_object_type(tmp.path(), "also_fridge.ron", FRIDGE_OK);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::DuplicateObjectTypeId { .. }),
        "got {err:?}"
    );
}

#[test]
fn duplicate_accessory_id_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
Accessory(
    id: AccessoryId(7),
    display_name: "Sunglasses",
    mesh_id: MeshId(101),
    slot: Head,
)
"#;
    write_accessory(tmp.path(), "a.ron", body);
    write_accessory(tmp.path(), "b.ron", body);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::DuplicateAccessoryId { .. }),
        "got {err:?}"
    );
}

#[test]
fn duplicate_advertisement_id_within_object_type_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: {},
    advertisements: [
        Advertisement(
            id: AdvertisementId(1), display_name: "A",
            preconditions: [], effects: [],
            duration_ticks: 1, interrupt_class: Never,
            score_template: ScoreTemplate(
                need_weights: [], personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
        Advertisement(
            id: AdvertisementId(1), display_name: "B",
            preconditions: [], effects: [],
            duration_ticks: 1, interrupt_class: Never,
            score_template: ScoreTemplate(
                need_weights: [], personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
"#;
    write_object_type(tmp.path(), "fridge.ron", body);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::DuplicateAdvertisementId { .. }),
        "got {err:?}"
    );
}

#[test]
fn unknown_object_state_key_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: { "stocked": Bool(true) },
    advertisements: [
        Advertisement(
            id: AdvertisementId(1), display_name: "A",
            preconditions: [ ObjectState("missing", Eq, Bool(true)) ],
            effects: [],
            duration_ticks: 1, interrupt_class: Never,
            score_template: ScoreTemplate(
                need_weights: [], personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
"#;
    write_object_type(tmp.path(), "fridge.ron", body);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::UnknownObjectStateKey { .. }),
        "got {err:?}"
    );
}

#[test]
fn duplicate_need_weight_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: {},
    advertisements: [
        Advertisement(
            id: AdvertisementId(1), display_name: "A",
            preconditions: [], effects: [],
            duration_ticks: 1, interrupt_class: Never,
            score_template: ScoreTemplate(
                need_weights: [(Hunger, 1.0), (Hunger, 0.5)],
                personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
"#;
    write_object_type(tmp.path(), "fridge.ron", body);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::DuplicateNeedWeight { .. }),
        "got {err:?}"
    );
}

#[test]
fn zero_duration_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: {},
    advertisements: [
        Advertisement(
            id: AdvertisementId(1), display_name: "A",
            preconditions: [], effects: [],
            duration_ticks: 0, interrupt_class: Never,
            score_template: ScoreTemplate(
                need_weights: [], personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
"#;
    write_object_type(tmp.path(), "fridge.ron", body);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(
        matches!(err, ContentError::ZeroDuration { .. }),
        "got {err:?}"
    );
}
```

- [ ] **Step 5.3: Run the failing tests**

```bash
cargo test -p gecko-sim-content --test validation
```

Expected: tests fail — `load_from_dir` currently accepts every-thing, so `expect_err` calls panic on `Ok`.

- [ ] **Step 5.4: Create `crates/content/src/validate.rs`**

```rust
//! Validators for the loaded content. Pure functions over the loader's
//! `Vec<(PathBuf, T)>` output. Validation runs after loading and before
//! the entries are collected into the `ContentBundle` maps so duplicate
//! detection sees both colliding paths.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gecko_sim_core::agent::Accessory;
use gecko_sim_core::ids::{AccessoryId, ObjectTypeId};
use gecko_sim_core::object::{ObjectType, Predicate};

use crate::error::ContentError;

pub(crate) fn validate_object_types(
    entries: &[(PathBuf, ObjectType)],
) -> Result<(), ContentError> {
    let mut seen: HashMap<ObjectTypeId, PathBuf> = HashMap::new();
    for (path, ot) in entries {
        if let Some(prev) = seen.get(&ot.id) {
            return Err(ContentError::DuplicateObjectTypeId {
                id: ot.id,
                first: prev.clone(),
                second: path.clone(),
            });
        }
        validate_advertisements(path, ot)?;
        seen.insert(ot.id, path.clone());
    }
    Ok(())
}

pub(crate) fn validate_accessories(
    entries: &[(PathBuf, Accessory)],
) -> Result<(), ContentError> {
    let mut seen: HashMap<AccessoryId, PathBuf> = HashMap::new();
    for (path, acc) in entries {
        if let Some(prev) = seen.get(&acc.id) {
            return Err(ContentError::DuplicateAccessoryId {
                id: acc.id,
                first: prev.clone(),
                second: path.clone(),
            });
        }
        seen.insert(acc.id, path.clone());
    }
    Ok(())
}

fn validate_advertisements(path: &PathBuf, ot: &ObjectType) -> Result<(), ContentError> {
    let mut seen_ads = HashSet::new();
    for ad in &ot.advertisements {
        if !seen_ads.insert(ad.id) {
            return Err(ContentError::DuplicateAdvertisementId {
                object_type: ot.id,
                ad: ad.id,
                path: path.clone(),
            });
        }
        if ad.duration_ticks == 0 {
            return Err(ContentError::ZeroDuration {
                object_type: ot.id,
                ad: ad.id,
                path: path.clone(),
            });
        }
        // ObjectState predicate keys must reference default_state.
        for predicate in &ad.preconditions {
            if let Predicate::ObjectState(key, _, _) = predicate {
                if !ot.default_state.contains_key(key) {
                    return Err(ContentError::UnknownObjectStateKey {
                        object_type: ot.id,
                        ad: ad.id,
                        key: key.clone(),
                        known: ot.default_state.keys().cloned().collect(),
                        path: path.clone(),
                    });
                }
            }
        }
        // No duplicate Need in score_template.need_weights.
        let mut seen_needs = HashSet::new();
        for (need, _) in &ad.score_template.need_weights {
            if !seen_needs.insert(*need) {
                return Err(ContentError::DuplicateNeedWeight {
                    object_type: ot.id,
                    ad: ad.id,
                    need: *need,
                    path: path.clone(),
                });
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 5.5: Wire validators into `load_from_dir`**

Replace the body of `pub fn load_from_dir` in `crates/content/src/lib.rs`:

```rust
pub fn load_from_dir(root: &Path) -> Result<ContentBundle, ContentError> {
    let object_types = loader::load_object_types(root)?;
    let accessories = loader::load_accessories(root)?;

    validate::validate_object_types(&object_types)?;
    validate::validate_accessories(&accessories)?;

    let mut bundle = ContentBundle::default();
    for (_, ot) in object_types {
        bundle.object_types.insert(ot.id, ot);
    }
    for (_, acc) in accessories {
        bundle.accessories.insert(acc.id, acc);
    }
    Ok(bundle)
}
```

And register the new module — add `mod validate;` near the existing `mod loader;` line in the same file.

- [ ] **Step 5.6: Run the validation tests**

```bash
cargo test -p gecko-sim-content --test validation
```

Expected: all 7 tests pass.

- [ ] **Step 5.7: Verify the smoke test still passes (good content still loads)**

```bash
cargo test -p gecko-sim-content
```

Expected: all tests across `loader_smoke`, `validation`, and the original `smoke` pass.

- [ ] **Step 5.8: Verify the workspace stays clean**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all green.

- [ ] **Step 5.9: Confirm commit**

```bash
jj st
```

Expected: `@` is "RON content: validators for unique IDs, state-key references, score-template sanity"; edits in `crates/content/src/{lib,validate}.rs` and `crates/content/tests/validation.rs`.

---

## Task 6: Seed catalog files at `content/`

**Files:**
- Create: `content/object_types/fridge.ron`
- Create: `content/object_types/chair.ron`
- Create: `content/accessories/sunglasses.ron`
- Create: `content/accessories/bowtie.ron`
- Modify: `content/README.md`
- Create: `crates/content/tests/seed_loads.rs`

- [ ] **Step 6.1: Start the task commit**

```bash
jj new -m "RON content: seed catalog (two object types, two accessories) at content/"
```

- [ ] **Step 6.2: Write the failing seed-load test**

Create `crates/content/tests/seed_loads.rs`:

```rust
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
```

- [ ] **Step 6.3: Run the failing test**

```bash
cargo test -p gecko-sim-content --test seed_loads
```

Expected: test fails because the seed `.ron` files don't exist yet (the loader returns an empty bundle when the subdirs are missing, so `assert_eq!(_, 2)` fails).

- [ ] **Step 6.4: Create `content/object_types/fridge.ron`**

```ron
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Fridge",
    mesh_id: MeshId(1),
    default_state: {
        "stocked": Bool(true),
    },
    advertisements: [
        Advertisement(
            id: AdvertisementId(1),
            display_name: "Eat snack",
            preconditions: [
                ObjectState("stocked", Eq, Bool(true)),
                AgentNeed(Hunger, Lt, 0.6),
            ],
            effects: [
                AgentNeedDelta(Hunger, 0.4),
            ],
            duration_ticks: 10,
            interrupt_class: NeedsThresholdOnly,
            score_template: ScoreTemplate(
                need_weights: [(Hunger, 1.0)],
                personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
```

- [ ] **Step 6.5: Create `content/object_types/chair.ron`**

```ron
ObjectType(
    id: ObjectTypeId(2),
    display_name: "Chair",
    mesh_id: MeshId(2),
    default_state: {},
    advertisements: [
        Advertisement(
            id: AdvertisementId(1),
            display_name: "Sit",
            preconditions: [],
            effects: [
                AgentNeedDelta(Comfort, 0.2),
            ],
            duration_ticks: 5,
            interrupt_class: Always,
            score_template: ScoreTemplate(
                need_weights: [(Comfort, 1.0)],
                personality_weights: Personality(
                    openness: 0.0, conscientiousness: 0.0,
                    extraversion: 0.0, agreeableness: 0.0, neuroticism: 0.0,
                ),
                situational_modifiers: [],
            ),
        ),
    ],
)
```

- [ ] **Step 6.6: Create `content/accessories/sunglasses.ron`**

```ron
Accessory(
    id: AccessoryId(1),
    display_name: "Sunglasses",
    mesh_id: MeshId(101),
    slot: Head,
)
```

- [ ] **Step 6.7: Create `content/accessories/bowtie.ron`**

```ron
Accessory(
    id: AccessoryId(2),
    display_name: "Bow tie",
    mesh_id: MeshId(102),
    slot: Neck,
)
```

- [ ] **Step 6.8: Refresh `content/README.md`**

Replace its current contents with:

```markdown
# Content

RON catalogs loaded by `gecko-sim-content::load_from_dir`. See ADR 0011
("Authoring format") for the schema and ADR 0012 ("crates/content") for
the loader's place in the architecture.

## Layout

```
content/
├── object_types/   one ObjectType per *.ron file
└── accessories/    one Accessory per *.ron file
```

Files within each subdirectory are loaded in lexicographic filename order
for deterministic results across platforms. Each file holds exactly one
top-level value of the appropriate type.

## Validation

The loader rejects:
- duplicate `ObjectTypeId` or `AccessoryId` across files
- duplicate `AdvertisementId` within a single `ObjectType`
- `Predicate::ObjectState` keys not present in the type's `default_state`
- duplicate `Need` entries in a `ScoreTemplate.need_weights`
- `duration_ticks: 0`
```

- [ ] **Step 6.9: Run the seed test**

```bash
cargo test -p gecko-sim-content --test seed_loads
```

Expected: passes.

- [ ] **Step 6.10: Verify the workspace stays clean**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all green.

- [ ] **Step 6.11: Confirm commit**

```bash
jj st
```

Expected: `@` is "RON content: seed catalog (two object types, two accessories) at content/"; edits in `content/{README.md, object_types/*.ron, accessories/*.ron}` and the new `crates/content/tests/seed_loads.rs`.

---

## Task 7: Host wires `GECKOSIM_CONTENT_DIR` into `Sim::new`

**Files:**
- Modify: `crates/host/src/config.rs`
- Modify: `crates/host/src/main.rs`
- Modify: `crates/host/Cargo.toml`

- [ ] **Step 7.1: Start the task commit**

```bash
jj new -m "RON content: host wires GECKOSIM_CONTENT_DIR into Sim::new"
```

- [ ] **Step 7.2: Write the failing config test**

Append to `crates/host/src/config.rs` inside the existing `#[cfg(test)] mod tests { … }` block:

```rust
    #[test]
    fn content_dir_default_ends_in_content() {
        let path = resolve_content_dir(None);
        assert!(
            path.ends_with("content"),
            "expected default to end in 'content', got {}",
            path.display()
        );
    }

    #[test]
    fn content_dir_override_uses_raw_path() {
        let path = resolve_content_dir(Some("/abs/path/to/elsewhere"));
        assert_eq!(path, std::path::PathBuf::from("/abs/path/to/elsewhere"));
    }
```

- [ ] **Step 7.3: Run the failing test**

```bash
cargo test -p gecko-sim-host --lib
```

Expected: compile error — `resolve_content_dir` doesn't exist.

- [ ] **Step 7.4: Implement `resolve_content_dir` and `content_dir` in `crates/host/src/config.rs`**

Append to the same file (above the existing `#[cfg(test)]` block):

```rust
use std::path::{Path, PathBuf};

const DEFAULT_CONTENT_SUBDIR: &str = "content";

/// Environment variable consulted by [`content_dir`].
pub const CONTENT_ENV_VAR: &str = "GECKOSIM_CONTENT_DIR";

/// Pure helper: resolve a content directory from `Some(env_value)` or fall
/// back to the workspace-relative default. Exposed for tests; production
/// calls go through [`content_dir`].
pub fn resolve_content_dir(raw: Option<&str>) -> PathBuf {
    if let Some(s) = raw {
        return PathBuf::from(s);
    }
    // CARGO_MANIFEST_DIR = <workspace>/crates/host
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(DEFAULT_CONTENT_SUBDIR)
}

/// Resolve the content directory from `GECKOSIM_CONTENT_DIR` or fall back
/// to `<workspace>/content`. Reads process env at call time.
pub fn content_dir() -> PathBuf {
    resolve_content_dir(std::env::var(CONTENT_ENV_VAR).ok().as_deref())
}
```

- [ ] **Step 7.5: Run the config tests**

```bash
cargo test -p gecko-sim-host --lib
```

Expected: existing `parse_addr` tests + the two new ones pass.

- [ ] **Step 7.6: Add `gecko-sim-content` as a host dependency**

In `crates/host/Cargo.toml`, find the `[dependencies]` block and add:

```toml
gecko-sim-content.workspace = true
```

(Place it next to the existing `gecko-sim-core.workspace = true` and `gecko-sim-protocol.workspace = true` lines.)

- [ ] **Step 7.7: Wire content loading into `crates/host/src/main.rs`**

The current `main.rs` is a thin entrypoint that uses `gecko_sim_host::{config, sim_driver, ws_server}` and binds its own `TcpListener`. Three precise edits — do not rewrite the whole file:

**Edit A.** Replace the import line for `gecko_sim_core` and add `anyhow::Context`. Find:

```rust
use gecko_sim_core::{ContentBundle, Sim};
```

Replace with:

```rust
use anyhow::Context;
use gecko_sim_core::Sim;
```

**Edit B.** Replace the `Sim::new` line with the loader-driven version. Find:

```rust
    let mut sim = Sim::new(DEMO_SEED, ContentBundle::default());
```

Replace with:

```rust
    let content_path = config::content_dir();
    tracing::info!(path = %content_path.display(), "loading content");
    let content = gecko_sim_content::load_from_dir(&content_path)
        .with_context(|| format!("loading content from {}", content_path.display()))?;
    tracing::info!(
        object_types = content.object_types.len(),
        accessories = content.accessories.len(),
        "content loaded"
    );

    let mut sim = Sim::new(DEMO_SEED, content);
```

After both edits, the file's structure (mod-style imports from `gecko_sim_host::{...}`, separate `TcpListener::bind` call, `ws_server::run(listener, ...)` arity) is unchanged from the WS pass. (Task 2 already removed the stale `#[expect]`/`#[allow]` clippy suppression that previously sat above `#[tokio::main]`, so there is no attribute to delete here.)

- [ ] **Step 7.8: Run the host tests**

```bash
cargo test -p gecko-sim-host
```

Expected: lib tests (config) pass; `tests/ws_smoke.rs` passes — it constructs `Sim::new(seed, ContentBundle::default())` directly, never going through the loader, so the content path doesn't matter for it.

- [ ] **Step 7.9: Verify the workspace stays clean**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all green.

- [ ] **Step 7.10: Manual smoke run — happy path**

```bash
cargo run -p gecko-sim-host
```

Expected output (then ctrl-c) — exact phrasing of unchanged log lines may carry tracing-formatter detail; the content-load lines are the new bit to look for:

```
gecko-sim host v0.1.0
loading content path=…/geckosim/crates/host/../../content
content loaded object_types=2 accessories=2
sim primed agents=3
ws transport listening local_addr=127.0.0.1:9001
^C
ctrl-c received, shutting down
```

(Optional further check: `wscat -c ws://127.0.0.1:9001/` continues to work exactly as in the WS pass.)

- [ ] **Step 7.11: Manual smoke run — env var override fail-fast**

```bash
GECKOSIM_CONTENT_DIR=/tmp/does-not-exist-xyz cargo run -p gecko-sim-host
```

Expected: process exits non-zero with an `Error: loading content from /tmp/does-not-exist-xyz` message; WS server never starts. The exact source-chain wording will vary by platform, but the path appears at least once in the output.

(Cleanup: `unset GECKOSIM_CONTENT_DIR` before re-running normally.)

- [ ] **Step 7.12: Confirm final commit**

```bash
jj st
jj log -r "ancestors(@, 8)" --no-graph | head -20
```

Expected: `@` is "RON content: host wires GECKOSIM_CONTENT_DIR into Sim::new"; the prior 6 commits in the log walk back through the task list; the spec commit (`lkowrlxy`) sits below them.

---

## Definition of done (rolled-up gate)

After Task 7 lands:

- `cargo build --workspace` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace` — all existing tests still pass; the new `catalogs`, `loader_smoke`, `validation`, `seed_loads`, and host config tests pass.
- Manual `cargo run -p gecko-sim-host` boots, logs `content loaded object_types=2 accessories=2`, listens on `127.0.0.1:9001`. WS smoke flow (`hello` → `init` → `snapshot` stream) is unchanged from the WS pass.
- `GECKOSIM_CONTENT_DIR=/bogus cargo run -p gecko-sim-host` fails with a clear anyhow trail naming the path.
- The `jj log` shows seven atomic commits matching the commit-strategy section of the spec.
