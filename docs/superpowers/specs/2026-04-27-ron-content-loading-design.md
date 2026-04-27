# RON content loading v0 — populate the catalog from disk

- **Date:** 2026-04-27
- **Status:** Draft
- **Scope:** Fourth implementation pass. Stands up the `gecko-sim-content` crate's RON loaders, makes `ContentBundle` non-empty, introduces an `Accessory` schema in `core`, and wires the host to load catalogs from disk before constructing `Sim`.
- **Predecessors:**
  - [`2026-04-26-scaffold-workspace-v0-design.md`](2026-04-26-scaffold-workspace-v0-design.md) — 4-crate workspace; `core` types from ADR 0011 (including `ObjectType` / `Advertisement` with serde derives); `gecko-sim-content` placeholder crate with `ron`/`serde`/`thiserror` deps already declared.
  - [`2026-04-26-live-runtime-v0-design.md`](2026-04-26-live-runtime-v0-design.md) — `Sim::new(seed, ContentBundle)` API; `bevy_ecs::World`-backed sim. `ContentBundle` is currently a unit struct in `core::sim`.
  - [`2026-04-27-ws-transport-v0-design.md`](2026-04-27-ws-transport-v0-design.md) — host wires `Sim::new(DEMO_SEED, ContentBundle::default())`; `host::config` resolves `GECKOSIM_HOST_ADDR`.

## Goal

End state: `cargo run -p gecko-sim-host` resolves a content directory (default `<workspace>/content`, override via `GECKOSIM_CONTENT_DIR`), loads RON files for object types and accessories, validates them, and constructs `Sim::new(seed, content)` with the populated bundle. `Sim` decomposes the bundle into two `bevy_ecs` resources — `ObjectCatalog` and `AccessoryCatalog` — that future scoring / rendering systems will read. The repo ships a small seed catalog (two object types, two accessories) so the loader has real content to chew on; the WS smoke test continues to pass against `ContentBundle::default()`.

This is the slice tagged "RON content pass" in the WS v0 follow-up list. It does not spawn instances of catalog types, does not retire `spawn_test_agent`, and does not consume the catalog from any system yet — those are explicitly the next pass.

## Non-goals (deferred — see "Deferred items" section)

- No instance spawning. `SmartObject` instances are not added to the world from the catalog. `spawn_test_agent` stays as the only entity-creation entry point.
- No system reads from `ObjectCatalog` / `AccessoryCatalog` yet. Resources are inserted but unread; this is fine — they're the contract for the next pass (second system / decision runtime).
- No content-driven agent generation (the "Agent generation" section of ADR 0011). Migration arrivals are a later pass.
- No hot reload. Content is loaded once at `Sim::new`; changing files at runtime requires a process restart.
- No save/load of the catalog. Catalogs are content (read-only); save data references catalog IDs, but ADR 0012 puts save/load in a separate pass.
- No frontend exposure of the catalog. ADR 0013 anticipates `ServerMessage::Init` carrying world structure + object catalog; that lands when the frontend pass first needs it.
- No cross-system soft validation (predicates referencing macro vars / event types against still-stub vocabularies).
- No accessory mechanics. Accessories remain aesthetic-only at v0 per ADR 0011. The new `Accessory` struct is purely descriptive (id, display name, mesh, slot).
- No tooling for content authoring (validators-as-CLI, schema export). The loader's `Result<ContentBundle, ContentError>` surface is the only authoring feedback channel; CLI wrappers can land later.

## Architecture

### Layout under `content/`

```
content/
├── README.md
├── object_types/
│   ├── chair.ron
│   └── fridge.ron
└── accessories/
    ├── bowtie.ron
    └── sunglasses.ron
```

Each `.ron` file holds **one** entry — a single `ObjectType` value or a single `Accessory` value. Files are discovered by enumerating each subdirectory non-recursively and filtering to entries whose extension is `ron`. Discovered paths are sorted lexicographically by filename before parsing, so load order is deterministic across platforms (macOS / Linux `read_dir` order is otherwise unspecified). Subdirectories under `object_types/` and `accessories/` are ignored at v0 — when the catalog grows large enough to want grouping (e.g. `object_types/kitchen/`), recursion can land as a non-breaking change.

### Crate dependency graph (unchanged)

```
host ──▶ content ──▶ core
   │                  ▲
   └──▶ protocol ─────┘
```

The loader returns `core::ContentBundle`. ADR 0012 already pins this direction (`content` depends on `core`, not vice versa).

### `ContentBundle` shape

`ContentBundle` moves from a unit struct to a value type with two indexed sub-catalogs. It stays in `core::sim` because the public `Sim::new` signature speaks `ContentBundle` and the dep graph forbids `core` from referring to anything in `content`.

```rust
// core::sim
use std::collections::HashMap;

use crate::agent::Accessory;          // new — see below
use crate::ids::{AccessoryId, ObjectTypeId};
use crate::object::ObjectType;

#[derive(Debug, Clone, Default)]
pub struct ContentBundle {
    pub object_types: HashMap<ObjectTypeId, ObjectType>,
    pub accessories: HashMap<AccessoryId, Accessory>,
}
```

`HashMap` is fine: catalogs are read-only after construction, and lookup is the dominant access pattern. Iteration order does not affect simulation determinism (see "Determinism" below).

`ContentBundle::default()` returns empty maps. The WS smoke test, the determinism test, and the live-runtime smoke test all keep using `ContentBundle::default()` unchanged.

### Resources inserted into the `World`

`Sim::new` decomposes the bundle on the way into ECS:

```rust
// core::object
#[derive(bevy_ecs::prelude::Resource, Debug, Clone, Default)]
pub struct ObjectCatalog {
    pub by_id: HashMap<ObjectTypeId, ObjectType>,
}

// core::agent
#[derive(bevy_ecs::prelude::Resource, Debug, Clone, Default)]
pub struct AccessoryCatalog {
    pub by_id: HashMap<AccessoryId, Accessory>,
}
```

`Sim::new` body:

```rust
pub fn new(seed: u64, content: ContentBundle) -> Self {
    let mut world = World::new();
    world.insert_resource(ObjectCatalog { by_id: content.object_types });
    world.insert_resource(AccessoryCatalog { by_id: content.accessories });
    Self { world, tick: 0, rng: PrngState::from_seed(seed), next_agent_id: 0 }
}
```

Two resources, not one, so a system that only needs object types doesn't take a `Res` lock on accessories (and vice versa). Both `Resource`s implement `Default` (empty maps) so future tests can spawn a `World` with empty catalogs by `World::init_resource`.

### `Accessory` schema (new in `core::agent`)

```rust
// core::agent
use serde::{Deserialize, Serialize};

use crate::ids::AccessoryId;
use crate::object::MeshId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessorySlot {
    Head,
    Neck,
    Body,
    Tail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Accessory {
    pub id: AccessoryId,
    pub display_name: String,
    pub mesh_id: MeshId,
    pub slot: AccessorySlot,
}
```

The four slot values match the four-slot cap on `Appearance.accessories` in ADR 0011. Slot is the one piece of structure renderers will inevitably need; everything else is deferred per the "aesthetic-only at v0" stance. `Appearance.accessories` (a `Vec<AccessoryId>`) is unchanged — slot lives on the catalog entry, not on the per-agent reference.

`MeshId` already exists in `core::object` and is `Serialize + Deserialize + Copy`. It is reused as-is.

### Loader public API (`gecko-sim-content`)

```rust
// crates/content/src/lib.rs
use std::path::Path;

use gecko_sim_core::ContentBundle;

pub mod error;
pub use error::ContentError;

pub fn load_from_dir(root: &Path) -> Result<ContentBundle, ContentError>;
```

Internally:

```rust
mod loader {
    pub(crate) fn load_object_types(dir: &Path) -> Result<Vec<(PathBuf, ObjectType)>, ContentError>;
    pub(crate) fn load_accessories(dir: &Path) -> Result<Vec<(PathBuf, Accessory)>, ContentError>;
}

mod validate {
    pub(crate) fn validate_object_types(entries: &[(PathBuf, ObjectType)]) -> Result<(), ContentError>;
    pub(crate) fn validate_accessories(entries: &[(PathBuf, Accessory)]) -> Result<(), ContentError>;
}
```

`load_from_dir` calls the loaders, then the validators, then collects into the `ContentBundle` maps. Returning `Vec<(PathBuf, T)>` from the loaders carries the source path of each entry through validation so error messages can cite both files when a duplicate is detected.

The loaders accept absent subdirectories gracefully: if `<root>/object_types/` does not exist, the result is an empty vec (and validation trivially passes). This keeps the `ContentBundle::default()` story uniform with `load_from_dir` over an empty directory.

### Error type

```rust
// crates/content/src/error.rs
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

    #[error("duplicate ObjectTypeId {id:?} in {first} and {second}")]
    DuplicateObjectTypeId {
        id: ObjectTypeId,
        first: PathBuf,
        second: PathBuf,
    },

    #[error("duplicate AccessoryId {id:?} in {first} and {second}")]
    DuplicateAccessoryId {
        id: AccessoryId,
        first: PathBuf,
        second: PathBuf,
    },

    #[error("duplicate AdvertisementId {ad:?} within ObjectType {object_type:?} in {path}")]
    DuplicateAdvertisementId {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        path: PathBuf,
    },

    #[error(
        "advertisement {ad:?} on ObjectType {object_type:?} ({path}) references unknown ObjectState key {key:?}; \
         known keys: {known:?}"
    )]
    UnknownObjectStateKey {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        key: StateKey,
        known: Vec<StateKey>,
        path: PathBuf,
    },

    #[error("advertisement {ad:?} on ObjectType {object_type:?} ({path}) repeats need_weight {need:?}")]
    DuplicateNeedWeight {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        need: Need,
        path: PathBuf,
    },

    #[error("advertisement {ad:?} on ObjectType {object_type:?} ({path}) has duration_ticks=0")]
    ZeroDuration {
        object_type: ObjectTypeId,
        ad: AdvertisementId,
        path: PathBuf,
    },
}
```

`ron::error::SpannedError` is what `ron::from_str` returns; it carries line/column info that surfaces directly in the `Display` rendering of the wrapped variant. `thiserror`'s `#[source]` chain lets `host`'s `anyhow` context surface the full cause when it bails.

### Validation rules (per Q4 / option B)

For each `ObjectType` entry:

1. **Unique `ObjectTypeId` across files.** Insert into `HashMap<ObjectTypeId, PathBuf>`; on collision, emit `DuplicateObjectTypeId` with both paths.
2. **Unique `AdvertisementId` within the type.** `HashSet<AdvertisementId>`; collision → `DuplicateAdvertisementId`.
3. **`ObjectState` predicate keys reference `default_state`.** For each `Predicate::ObjectState(key, _, _)` in each advertisement, assert `default_state.contains_key(key)`. Other predicate variants (`AgentNeed`, `AgentSkill`, …) are not validated here — their referents live in the agent, not the object type.
4. **No duplicate `Need` in `score_template.need_weights`.** A duplicate means an authoring typo; no defined behaviour for two weights on the same need.
5. **`duration_ticks > 0`.** A zero-duration advertisement triggers no end-tick (action commitment expects `expected_end_tick > started_tick`).

For each `Accessory` entry:

1. **Unique `AccessoryId` across files.** Same pattern as object types.

Validation runs **after** all entries from one subdirectory are loaded (so duplicate detection sees both sides) and **before** the entries are collected into the `ContentBundle` maps. A failed validation returns the first error encountered; reporting all errors at once is a future quality-of-life improvement.

### Determinism

ADR 0008 makes the simulation deterministic on the world seed. The loader path is part of `Sim` construction, not a per-tick code path, so iteration order over `HashMap<…, ObjectType>` does not affect tick reproducibility. The catalog-as-input is a property the determinism contract treats as fixed: same content + same seed = same trace. This pass introduces no new sources of nondeterminism.

The loader itself produces deterministic output (sorted file lists; deterministic validation order) so two runs over the same `content/` directory build byte-equal `ContentBundle`s up to `HashMap` collision order — fine, because no consumer iterates the maps yet.

### Host wiring

Add a sibling to `host::config::listen_addr`:

```rust
// host::config
const DEFAULT_CONTENT_SUBDIR: &str = "content";
pub const CONTENT_ENV_VAR: &str = "GECKOSIM_CONTENT_DIR";

pub fn content_dir() -> PathBuf {
    if let Ok(s) = env::var(CONTENT_ENV_VAR) {
        return PathBuf::from(s);
    }
    // Workspace-relative fallback: <crate manifest dir>/../../content
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(DEFAULT_CONTENT_SUBDIR)
}
```

`env!("CARGO_MANIFEST_DIR")` evaluates at compile time to `<workspace>/crates/host`; the `../../content` prefix lands at `<workspace>/content`. This makes `cargo run -p gecko-sim-host` work regardless of the user's cwd — important because the previous pass's manual smoke (`cargo run -p gecko-sim-host`) is the one path that exercises the loader end-to-end. Override via `GECKOSIM_CONTENT_DIR=/abs/path` for production deployments.

`host::main` becomes:

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
sim.spawn_test_agent("Alice");
sim.spawn_test_agent("Bob");
sim.spawn_test_agent("Charlie");
// ... rest unchanged from WS pass.
```

Anyhow context wraps the typed `ContentError`: the context names the content root, and the source chain names the offending file (and its line/column for `Parse`). Two complementary frames, not redundant.

## Module changes by crate

### `gecko-sim-core`

- **Modified:**
  - `src/sim.rs` — `ContentBundle` becomes `pub struct ContentBundle { pub object_types: HashMap<…>, pub accessories: HashMap<…> }` with `Default`. `Sim::new` decomposes it into `ObjectCatalog` / `AccessoryCatalog` resources via `World::insert_resource`.
  - `src/agent/mod.rs` — adds `pub enum AccessorySlot { Head, Neck, Body, Tail }`, `pub struct Accessory { id, display_name, mesh_id, slot }`, and `pub struct AccessoryCatalog { pub by_id: HashMap<AccessoryId, Accessory> }` with `Resource + Default` derives.
  - `src/object/mod.rs` — adds `pub struct ObjectCatalog { pub by_id: HashMap<ObjectTypeId, ObjectType> }` with `Resource + Default` derives.
  - `src/lib.rs` — re-exports `Accessory`, `AccessorySlot`, `AccessoryCatalog`, `ObjectCatalog`. `ContentBundle` is already re-exported.
- **New:** none.
- **Cargo.toml:** unchanged.
- **Untouched:** `systems/`, `world/`, `decision/`, `macro_/`, `events/`, `time/`, `save/`, `rng/`, `snapshot.rs`, `ids.rs`.

### `gecko-sim-content`

- **New:**
  - `src/error.rs` — the `ContentError` enum above.
  - `src/loader.rs` — private `load_object_types` / `load_accessories` over a subdirectory, returning `Vec<(PathBuf, T)>`. Each opens the file, reads to a `String`, and `ron::from_str::<T>(&contents)` into the typed value.
  - `src/validate.rs` — private `validate_object_types` / `validate_accessories`. Pure functions over the loaded vecs; no I/O.
- **Modified:**
  - `src/lib.rs` — declares the modules, re-exports `ContentError`, exposes `pub fn load_from_dir(root: &Path) -> Result<ContentBundle, ContentError>`. Drops the placeholder doc-comment-only contents.
- **Cargo.toml:** unchanged. `dev-dependencies`: add `tempfile.workspace = true` (new workspace dep — see "Workspace deps" below).

### `gecko-sim-protocol`

- **Untouched.**

### `gecko-sim-host`

- **Modified:**
  - `src/config.rs` — adds `CONTENT_ENV_VAR` const, `content_dir()` function, and a unit test that asserts the default resolves to a path ending in `/content` (no need to verify it exists — that's exercised by the seed test in `content`).
  - `src/main.rs` — calls `config::content_dir()`, `gecko_sim_content::load_from_dir(&path)`, threads the result into `Sim::new`. Adds a `tracing::info!` line for the loaded counts. Three-line addition.
- **Cargo.toml:** add `gecko-sim-content.workspace = true` to `[dependencies]` (it is currently absent because the host had no need of content). The workspace `gecko-sim-content` dep already exists in the workspace dependencies table.
- **Untouched:** `src/lib.rs`, `src/sim_driver.rs`, `src/ws_server.rs`, `tests/ws_smoke.rs`. The smoke test continues to call `Sim::new(seed, ContentBundle::default())` directly — it does not need to go through the loader.

### Workspace `Cargo.toml`

- **Modified:** add `tempfile = "3"` to `[workspace.dependencies]`. Used by the content-crate tests for tempdir construction. No production crate consumes it.

## Seed content

Two `ObjectType`s and two `Accessory`s, hand-authored, hitting the validation paths:

`content/object_types/fridge.ron`:

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

`content/object_types/chair.ron`:

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

`content/accessories/sunglasses.ron`:

```ron
Accessory(
    id: AccessoryId(1),
    display_name: "Sunglasses",
    mesh_id: MeshId(101),
    slot: Head,
)
```

`content/accessories/bowtie.ron`:

```ron
Accessory(
    id: AccessoryId(2),
    display_name: "Bow tie",
    mesh_id: MeshId(102),
    slot: Neck,
)
```

These are authoring templates as much as test fixtures. The fridge exercises `ObjectState` predicate validation against `default_state`; the chair has an empty `default_state` and zero `ObjectState` predicates. The two accessories cover two distinct slot variants.

`content/README.md` is updated from "loaders later" to a short description of the layout and link to ADR 0011.

## Tests

### `crates/content/tests/loader_smoke.rs`

Build a tempdir mirroring the seed layout (small inline RON strings, two object types, one accessory). Call `load_from_dir`; assert the bundle has exactly the expected counts and that ID lookups return the parsed entries. Asserts the happy path end-to-end without depending on the on-disk seed.

### `crates/content/tests/validation.rs`

One test per `ContentError` variant that the validator can produce. Each test writes a tempdir with the minimum content needed to trigger the variant, calls `load_from_dir`, and asserts the returned `Err` matches by variant via `assert!(matches!(err, ContentError::DuplicateObjectTypeId { .. }))`. Variants covered: `Parse` (intentionally malformed RON), `DuplicateObjectTypeId`, `DuplicateAccessoryId`, `DuplicateAdvertisementId`, `UnknownObjectStateKey`, `DuplicateNeedWeight`, `ZeroDuration`. `Io` is covered indirectly by passing a nonexistent root path with the subdir present-but-unreadable; if the OS-level permission setup is awkward, this case is dropped — the variant still exists for production use.

### `crates/content/tests/seed_loads.rs`

Resolve the workspace-root `content/` directory via `env!("CARGO_MANIFEST_DIR")` joined with `../../content`, call `load_from_dir`, assert the bundle contains exactly two object types and two accessories with the expected IDs. Locks the contract that the seed catalog stays in sync with the schema across schema-evolving passes.

### `crates/host/src/config.rs` unit test

Mirror the `parse_addr` split: refactor into a pure helper `resolve_content_dir(raw: Option<&str>) -> PathBuf` and a thin `content_dir()` wrapper that reads the env var. Test the pure helper — `None` returns a path ending in `content`; `Some("/abs/path")` returns exactly that path. Avoids `set_var` in tests, which races other tests in the same binary.

### Existing tests

- `crates/core/tests/{smoke,needs_decay,determinism}.rs` — pass unchanged. They construct `Sim::new(seed, ContentBundle::default())`, which still compiles and means "empty catalogs".
- `crates/host/tests/ws_smoke.rs` — passes unchanged for the same reason.
- `crates/content/tests/smoke.rs` — already a `dep_chain_resolves` no-op asserting the crate links to `core`. Stays.
- `crates/protocol/tests/roundtrip.rs` — untouched.

## Definition of done

- `cargo build --workspace` clean.
- `cargo test --workspace` — all existing tests pass; the new content loader / validation / seed tests pass; the host config test passes.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo run -p gecko-sim-host` boots, logs `loading content path=…/content`, `content loaded object_types=2 accessories=2`, then proceeds to listen on `127.0.0.1:9001` exactly as before. Manual `wscat -c ws://127.0.0.1:9001/` shows the same `hello` → `init` → `snapshot` stream as the WS pass.
- Setting `GECKOSIM_CONTENT_DIR` to a nonexistent path causes `cargo run -p gecko-sim-host` to fail with a non-zero exit and an anyhow-formatted error trail naming the path.
- One atomic `jj` commit on a topic branch (or several; the existing convention is per-task commits — see "Commit strategy" below).

## Commit strategy

Following the per-task pattern from prior passes, the work splits into roughly seven commits:

1. `RON content: Accessory, AccessorySlot, AccessoryCatalog, ObjectCatalog types`
2. `RON content: ContentBundle holds populated catalogs; Sim::new inserts resources`
3. `RON content: ContentError variants in gecko-sim-content`
4. `RON content: file-globbing loaders for object_types/ and accessories/`
5. `RON content: validators for unique IDs, state-key references, score-template sanity`
6. `RON content: seed catalog (two object types, two accessories) at content/`
7. `RON content: host wires GECKOSIM_CONTENT_DIR into Sim::new`

Each commit lands with its own tests where applicable. Plan-author may merge adjacent commits; the spec does not care.

## Trace to ADRs

- **ADR 0011 (schema):** authoring format section. RON for object catalog and accessory catalog, hot reload deferred. The `Accessory` shape (id / display_name / mesh_id / slot) is a strict subset of the "aesthetic-only" v0 stance — slot is the smallest renderer-relevant addition.
- **ADR 0012 (architecture):** the loader lives in `crates/content`; `core` does not depend on `content`. `ObjectCatalog` and `AccessoryCatalog` are `bevy_ecs::Resource`s so future systems take `Res<…>`. Errors are `thiserror` in `content`; host bridges to `anyhow`.
- **ADR 0013 (transport):** untouched. The catalog is not on the wire yet; the WS smoke flow is unchanged.
- **ADR 0008 (time):** untouched. Loader runs once at construction; no per-tick implication.

## Deferred items (carry forward to later passes)

| Item | Triggers landing | Lives in |
|---|---|---|
| Spawn `SmartObject` instances from the catalog | World-seed / scenario pass | `core::sim` (new constructor) + `host::main` |
| Retire `spawn_test_agent` in favour of content-driven agent generation | Migration arrivals (per ADR 0011) | `core::sim` + new agent-gen module |
| Hot reload | Author productivity pressure post-frontend | `host::main` (file-watcher) + `Sim::reload_content` |
| Multi-error reporting (collect-then-fail) | Authoring catalog grows past handfuls | `gecko-sim-content::validate` |
| Cross-system soft validation (macro-var well-typedness, event-payload variants) | `macro_/` and `events/` grow real vocabularies | `gecko-sim-content::validate` |
| Recursive subdirectories under `object_types/` and `accessories/` | Catalog grows large enough to want grouping | `gecko-sim-content::loader` |
| World structure + object catalog in `ServerMessage::Init` | Frontend pass | `protocol::messages` + `host::ws_server` |
| Save format that round-trips catalog refs | Save/load pass | `core::save` + `gecko-sim-content` (verifying catalog compatibility on load) |
| CLI `gecko-content-validate <path>` wrapper | Authoring workflow polish | New binary in `crates/content` |

## What this pass enables next

With catalogs populated and resources inserted into the `World`, two passes are unblocked:

1. **Second-system pass (`mood`).** Picks the next system from ADR 0010. Trivially independent of the catalog, but introduces the `bevy_ecs::Schedule` ceremony that future catalog-consuming systems will need.
2. **Decision-runtime pass.** First system that takes `Res<ObjectCatalog>` — e.g. a stub utility scorer that iterates `ObjectCatalog.by_id` and scores advertisements against an agent's `Needs`. This is where the catalog stops being inert.
