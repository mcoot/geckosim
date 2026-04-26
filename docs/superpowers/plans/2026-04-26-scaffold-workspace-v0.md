# v0 Scaffolding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scaffold the gecko-sim Rust workspace per ADR 0012 and translate the v0 schema from ADR 0011 into compilable Rust types — single atomic commit, lint-clean.

**Architecture:** 4-crate Cargo workspace (`gecko-sim-core`, `gecko-sim-content`, `gecko-sim-protocol`, `gecko-sim-host`) under `crates/`. `core` owns the schema (no I/O); `content` will load RON later; `protocol` will define wire types later; `host` is the binary entry point. ECS sharding, RON loaders, WS server, and ts-rs all deferred — this pass is types-only.

**Tech Stack:** Rust 2021, `bevy_ecs` 0.16 (imported but unused), `serde`, `thiserror`, `glam`, `rand_pcg`, `tracing`, `anyhow`. Tooling: `cargo`, `rustfmt`, `clippy`. VCS: `jj` (colocated with git).

**Source spec:** [docs/superpowers/specs/2026-04-26-scaffold-workspace-v0-design.md](../specs/2026-04-26-scaffold-workspace-v0-design.md)

---

## Notes for the implementer

- **VCS is `jj`, not raw `git`.** This repo is `jj`-tracked (colocated with `.git/`). Use `jj` commands. The "jujutsu" skill has the full reference.
- **Single atomic commit, not per-task commits.** The spec mandates one commit at completion. Use `jj new` once at Task 1 and let auto-snapshot accumulate changes into that single change. Verify state with `jj st` between tasks; do not run `jj describe` again, do not create new changes.
- **Build incrementally.** After each task, run the listed verification command. Stop and debug if it fails — do not continue stacking changes on a broken build.
- **Faithful translation, no improvisation.** Types come from ADR 0011 verbatim (with the alias decisions in the spec). If something looks ambiguous, re-read 0011 — do not invent new fields or rename existing ones. Document "deferred" choices in code comments only when 0011 explicitly defers them.
- **Versions in this plan are best-known-good as of 2026-04-26.** If `cargo build` complains about resolution, bump to whatever resolves cleanly within the same major version, and note it in the final commit message.

---

## File map (post-pass)

```
gecko-sim/
├── Cargo.toml                              ← Task 1
├── rustfmt.toml                            ← Task 1
├── .gitignore                              ← Task 1
├── crates/
│   ├── core/
│   │   ├── Cargo.toml                      ← Task 1
│   │   ├── src/lib.rs                      ← Task 1 (placeholder), Task 11 (re-exports)
│   │   ├── src/ids.rs                      ← Task 2
│   │   ├── src/world/mod.rs                ← Task 3
│   │   ├── src/rng/mod.rs                  ← Task 4
│   │   ├── src/time/mod.rs                 ← Task 5
│   │   ├── src/agent/mod.rs                ← Task 6
│   │   ├── src/object/mod.rs               ← Task 7
│   │   ├── src/decision/mod.rs             ← Task 8
│   │   ├── src/macro_/mod.rs               ← Task 9
│   │   ├── src/events/mod.rs               ← Task 9
│   │   ├── src/systems/mod.rs              ← Task 9
│   │   ├── src/save/mod.rs                 ← Task 9
│   │   └── tests/smoke.rs                  ← Task 11
│   ├── content/
│   │   ├── Cargo.toml                      ← Task 1
│   │   ├── src/lib.rs                      ← Task 1
│   │   └── tests/smoke.rs                  ← Task 12
│   ├── protocol/
│   │   ├── Cargo.toml                      ← Task 1
│   │   ├── src/lib.rs                      ← Task 1
│   │   └── tests/smoke.rs                  ← Task 12
│   └── host/
│       ├── Cargo.toml                      ← Task 1
│       └── src/main.rs                     ← Task 1 (placeholder), Task 13 (real)
├── content/README.md                       ← Task 1
└── apps/web/README.md                      ← Task 1
```

---

## Task 1: Workspace skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `rustfmt.toml`
- Create: `.gitignore`
- Create: `crates/core/Cargo.toml`
- Create: `crates/core/src/lib.rs`
- Create: `crates/content/Cargo.toml`
- Create: `crates/content/src/lib.rs`
- Create: `crates/protocol/Cargo.toml`
- Create: `crates/protocol/src/lib.rs`
- Create: `crates/host/Cargo.toml`
- Create: `crates/host/src/main.rs`
- Create: `content/README.md`
- Create: `apps/web/README.md`

**Goal:** Empty workspace builds. All four crates exist as compilable shells with the right deps wired. Single jj change is opened and described.

- [ ] **Step 1: Open a fresh jj change for the scaffold work**

```bash
jj new
jj desc -m "Scaffold workspace and v0 schema types from 0011/0012"
jj st
```

Expected: `jj st` shows working copy with no changes yet, `@` description matches.

- [ ] **Step 2: Write workspace root `Cargo.toml`**

`/Users/joseph/src/geckosim/Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/core", "crates/content", "crates/protocol", "crates/host"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
authors = ["gecko-sim contributors"]
repository = "https://github.com/jspearritt/geckosim"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
bevy_ecs = "0.16"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
glam = { version = "0.30", features = ["serde"] }
rand = "0.9"
rand_pcg = { version = "0.9", features = ["serde"] }
ron = "0.10"
anyhow = "1"

gecko-sim-core = { path = "crates/core" }
gecko-sim-content = { path = "crates/content" }
gecko-sim-protocol = { path = "crates/protocol" }

[workspace.lints.rust]
unsafe_code = "forbid"
warnings = "deny"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
must_use_candidate = "allow"
```

- [ ] **Step 3: Write `rustfmt.toml`**

`/Users/joseph/src/geckosim/rustfmt.toml`:

```toml
edition = "2021"
max_width = 100
```

- [ ] **Step 4: Write `.gitignore`**

`/Users/joseph/src/geckosim/.gitignore`:

```
/target
.DS_Store
```

- [ ] **Step 5: Write `crates/core/Cargo.toml`**

`/Users/joseph/src/geckosim/crates/core/Cargo.toml`:

```toml
[package]
name = "gecko-sim-core"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Gecko-sim core: schema types and (later) ECS-based simulation engine."

[dependencies]
serde.workspace = true
thiserror.workspace = true
bevy_ecs.workspace = true
glam.workspace = true
rand.workspace = true
rand_pcg.workspace = true

[lints]
workspace = true
```

- [ ] **Step 6: Write `crates/core/src/lib.rs` (placeholder)**

`/Users/joseph/src/geckosim/crates/core/src/lib.rs`:

```rust
//! Gecko-sim core: schema types and (later) the ECS-based simulation engine.
//!
//! At this scaffold pass, only the v0 schema from ADR 0011 is implemented.
//! The live `Sim` API, ECS components, and systems land in later passes.
```

- [ ] **Step 7: Write `crates/content/Cargo.toml`**

`/Users/joseph/src/geckosim/crates/content/Cargo.toml`:

```toml
[package]
name = "gecko-sim-content"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Gecko-sim content: RON loaders for object/accessory catalogs (later)."

[dependencies]
gecko-sim-core.workspace = true
serde.workspace = true
thiserror.workspace = true
ron.workspace = true

[lints]
workspace = true
```

- [ ] **Step 8: Write `crates/content/src/lib.rs`**

`/Users/joseph/src/geckosim/crates/content/src/lib.rs`:

```rust
//! Gecko-sim content: RON catalog loaders. Empty at scaffold pass.
```

- [ ] **Step 9: Write `crates/protocol/Cargo.toml`**

`/Users/joseph/src/geckosim/crates/protocol/Cargo.toml`:

```toml
[package]
name = "gecko-sim-protocol"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Gecko-sim protocol: wire types shared with the frontend (later)."

[dependencies]
gecko-sim-core.workspace = true
serde.workspace = true

[lints]
workspace = true
```

- [ ] **Step 10: Write `crates/protocol/src/lib.rs`**

`/Users/joseph/src/geckosim/crates/protocol/src/lib.rs`:

```rust
//! Gecko-sim protocol: wire types for the host ↔ frontend WebSocket channel.
//! Empty at scaffold pass — populated alongside the frontend wiring pass.
```

- [ ] **Step 11: Write `crates/host/Cargo.toml`**

`/Users/joseph/src/geckosim/crates/host/Cargo.toml`:

```toml
[package]
name = "gecko-sim-host"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Gecko-sim host: native binary that runs the sim and (later) serves WebSocket clients."

[dependencies]
gecko-sim-core.workspace = true
gecko-sim-content.workspace = true
gecko-sim-protocol.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow.workspace = true

[lints]
workspace = true
```

- [ ] **Step 12: Write `crates/host/src/main.rs` (placeholder)**

`/Users/joseph/src/geckosim/crates/host/src/main.rs`:

```rust
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
```

(Task 13 will revisit this; the placeholder is final-shape-correct so the verification command passes today.)

- [ ] **Step 13: Write `content/README.md`**

`/Users/joseph/src/geckosim/content/README.md`:

```markdown
# Content

RON catalogs (smart objects, accessories) live here once the loader pass lands. See ADR 0011 ("Authoring format") and ADR 0012 ("crates/content").
```

- [ ] **Step 14: Write `apps/web/README.md`**

`/Users/joseph/src/geckosim/apps/web/README.md`:

```markdown
# apps/web

Next.js / Three.js frontend. Not scaffolded yet — see ADR 0013. The frontend pass adds a real Next.js app under this directory.
```

- [ ] **Step 15: Verify the workspace builds**

```bash
cd /Users/joseph/src/geckosim && cargo build --workspace
```

Expected: a long fetch+compile (first-time deps). Final line: `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in <X>s` with no warnings.

If clippy or compile warnings appear, fix them before continuing — `warnings = "deny"` is on.

- [ ] **Step 16: Verify host binary runs**

```bash
cd /Users/joseph/src/geckosim && cargo run -p gecko-sim-host
```

Expected: a single tracing log line `... INFO gecko_sim_host: gecko-sim host v0.1.0`, then exit 0.

- [ ] **Step 17: Snapshot check**

```bash
jj st
```

Expected: working copy shows the 13 created files; description still `Scaffold workspace and v0 schema types from 0011/0012`.

---

## Task 2: Identifier newtypes

**Files:**
- Create: `crates/core/src/ids.rs`
- Modify: `crates/core/src/lib.rs`

**Goal:** Define every `*Id` newtype from ADR 0011 plus `OwnerRef`.

- [ ] **Step 1: Write `crates/core/src/ids.rs`**

`/Users/joseph/src/geckosim/crates/core/src/ids.rs`:

```rust
//! Stable u64-newtype identifiers per ADR 0011.
//!
//! These are the canonical IDs used in saves, the wire protocol, and all
//! cross-references between sim entities. ECS `Entity` handles are not
//! serialized — see ADR 0012 ("Identity surfaces").

use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        pub struct $name(pub u64);

        impl $name {
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn raw(self) -> u64 {
                self.0
            }
        }
    };
}

id_newtype!(AgentId);
id_newtype!(ObjectId);
id_newtype!(ObjectTypeId);
id_newtype!(BuildingId);
id_newtype!(LeafAreaId);
id_newtype!(HousingId);
id_newtype!(EmploymentId);
id_newtype!(HouseholdId);
id_newtype!(BusinessId);
id_newtype!(CrimeIncidentId);
id_newtype!(MemoryEntryId);
id_newtype!(AccessoryId);
id_newtype!(AdvertisementId);
id_newtype!(PromotedEventId);

/// An owner reference for entities that can be owned by an agent, household, or business
/// (e.g. a fridge belongs to a household; a register belongs to a business).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OwnerRef {
    Agent(AgentId),
    Household(HouseholdId),
    Business(BusinessId),
}
```

- [ ] **Step 2: Wire the module in `lib.rs`**

Replace the placeholder body of `/Users/joseph/src/geckosim/crates/core/src/lib.rs` with:

```rust
//! Gecko-sim core: schema types and (later) the ECS-based simulation engine.
//!
//! At this scaffold pass, only the v0 schema from ADR 0011 is implemented.
//! The live `Sim` API, ECS components, and systems land in later passes.

pub mod ids;
```

- [ ] **Step 3: Verify it compiles**

```bash
cd /Users/joseph/src/geckosim && cargo check -p gecko-sim-core
```

Expected: clean. No warnings.

---

## Task 3: World primitives — `Vec2`, `Color`

**Files:**
- Create: `crates/core/src/world/mod.rs`
- Modify: `crates/core/src/lib.rs`

**Goal:** Cross-module primitives used by agents and objects: `Vec2` (re-export from `glam`) and `Color`.

- [ ] **Step 1: Write `crates/core/src/world/mod.rs`**

`/Users/joseph/src/geckosim/crates/core/src/world/mod.rs`:

```rust
//! World-level primitives.
//!
//! At v0 this module is intentionally small: just the math/color primitives
//! used by other modules. The hierarchical spatial graph from ADR 0007
//! (district → building → floor → room → zone) lands here in a later pass.

use serde::{Deserialize, Serialize};

/// 2D position vector, re-exported from `glam`. Used for in-leaf-area positions
/// (snapped to a 0.5m grid for object alignment per ADR 0007).
pub use glam::Vec2;

/// 24-bit RGB color. No alpha, no HDR — appearance is pure 8-bit RGB at v0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}
```

- [ ] **Step 2: Add module to `lib.rs`**

In `/Users/joseph/src/geckosim/crates/core/src/lib.rs`, append after `pub mod ids;`:

```rust
pub mod world;
```

- [ ] **Step 3: Verify**

```bash
cd /Users/joseph/src/geckosim && cargo check -p gecko-sim-core
```

Expected: clean.

---

## Task 4: RNG state

**Files:**
- Create: `crates/core/src/rng/mod.rs`
- Modify: `crates/core/src/lib.rs`

**Goal:** `PrngState` newtype around `rand_pcg::Pcg64Mcg` for ADR 0008's seeded sub-stream pattern.

- [ ] **Step 1: Write `crates/core/src/rng/mod.rs`**

`/Users/joseph/src/geckosim/crates/core/src/rng/mod.rs`:

```rust
//! Seeded PRNG state per ADR 0008.
//!
//! Each agent gets its own seeded sub-stream from the world seed; saves
//! preserve PRNG state so replays are deterministic.

use serde::{Deserialize, Serialize};

/// Per-agent (or per-stream) PRNG state. Wraps `rand_pcg::Pcg64Mcg` because
/// it's deterministic, has small state, and is fast — appropriate for a
/// per-agent sub-stream that gets stepped many times per tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrngState(pub rand_pcg::Pcg64Mcg);

impl PrngState {
    /// Construct a fresh PRNG seeded with the given 64-bit seed.
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        use rand::SeedableRng;
        Self(rand_pcg::Pcg64Mcg::seed_from_u64(seed))
    }
}
```

- [ ] **Step 2: Add module to `lib.rs`**

Append `pub mod rng;` to `crates/core/src/lib.rs`.

- [ ] **Step 3: Verify**

```bash
cd /Users/joseph/src/geckosim && cargo check -p gecko-sim-core
```

Expected: clean.

---

## Task 5: Time primitives

**Files:**
- Create: `crates/core/src/time/mod.rs`
- Modify: `crates/core/src/lib.rs`

**Goal:** A `Tick` newtype and the sim-time constants from ADR 0008 (1 sim-minute per tick, 60 ticks per sim-hour).

- [ ] **Step 1: Write `crates/core/src/time/mod.rs`**

`/Users/joseph/src/geckosim/crates/core/src/time/mod.rs`:

```rust
//! Simulation time primitives per ADR 0008.

use serde::{Deserialize, Serialize};

/// One micro-tick of simulation. ADR 0008 fixes 1 tick = 1 sim-minute.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct Tick(pub u64);

impl Tick {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Number of micro-ticks in one sim-hour (per ADR 0008).
pub const TICKS_PER_SIM_HOUR: u64 = 60;

/// Number of micro-ticks in one sim-day.
pub const TICKS_PER_SIM_DAY: u64 = TICKS_PER_SIM_HOUR * 24;

/// Macro tick cadence per ADR 0009: one macro tick per sim-hour.
pub const TICKS_PER_MACRO_TICK: u64 = TICKS_PER_SIM_HOUR;
```

- [ ] **Step 2: Add module to `lib.rs`**

Append `pub mod time;` to `crates/core/src/lib.rs`.

- [ ] **Step 3: Verify**

```bash
cd /Users/joseph/src/geckosim && cargo check -p gecko-sim-core
```

Expected: clean.

---

## Task 6: Agent (gecko) schema

**Files:**
- Create: `crates/core/src/agent/mod.rs`
- Modify: `crates/core/src/lib.rs`

**Goal:** Translate the `Gecko` struct and all supporting types from ADR 0011 into Rust. Includes the dimension enums (`Need`, `Skill`, `MoodDim`, `RelField`) that other modules will reference.

Note on bounded collections: ADR 0011 specifies `BoundedRing<T, N>`, `BoundedVec<T, N>`, and `SparseMap<K, V>`. Per the spec, these are aliased to `Vec<T>` and `HashMap<K, V>` for v0. The bound from 0011 is recorded in a doc comment on the field where it appears.

- [ ] **Step 1: Write `crates/core/src/agent/mod.rs`**

`/Users/joseph/src/geckosim/crates/core/src/agent/mod.rs`:

```rust
//! Agent (gecko) schema per ADR 0011.
//!
//! At this scaffold pass, `Gecko` is a single monolithic struct. Sharding
//! into ECS components (`Needs`, `Personality`, `Mood`, …) happens in the
//! next pass when the live `Sim` API lands.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ids::{
    AccessoryId, AgentId, CrimeIncidentId, EmploymentId, HouseholdId, HousingId, MemoryEntryId,
    ObjectId,
};
use crate::rng::PrngState;
use crate::world::{Color, Vec2};

// ---------------------------------------------------------------------------
// Identity / appearance
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gender {
    Male,
    Female,
    NonBinary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pattern {
    Solid,
    Stripes,
    Spots,
    Mottled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Size {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TailLength {
    Short,
    Medium,
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntrinsicAppearance {
    pub base_color: Color,
    pub pattern: Pattern,
    pub pattern_color: Color,
    pub eye_color: Color,
    pub body_size: Size,
    pub tail_length: TailLength,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    pub intrinsic: IntrinsicAppearance,
    /// Bounded at 4 accessory slots per ADR 0011.
    pub accessories: Vec<AccessoryId>,
}

// ---------------------------------------------------------------------------
// (1) Needs
// ---------------------------------------------------------------------------

/// Need dimension. Cross-cutting — referenced by `Predicate::AgentNeed` and
/// `Effect::AgentNeedDelta` in the object module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Need {
    Hunger,
    Sleep,
    Social,
    Hygiene,
    Fun,
    Comfort,
}

/// All six need values, each in `[0, 1]`. Per ADR 0011.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Needs {
    pub hunger: f32,
    pub sleep: f32,
    pub social: f32,
    pub hygiene: f32,
    pub fun: f32,
    pub comfort: f32,
}

// ---------------------------------------------------------------------------
// (2) Personality — Big Five, components in [-1, 1]
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Personality {
    pub openness: f32,
    pub conscientiousness: f32,
    pub extraversion: f32,
    pub agreeableness: f32,
    pub neuroticism: f32,
}

// ---------------------------------------------------------------------------
// (3) Mood
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MoodDim {
    Valence,
    Arousal,
    Stress,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mood {
    pub valence: f32,
    pub arousal: f32,
    pub stress: f32,
}

// ---------------------------------------------------------------------------
// (4) Memory
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryKind {
    SocialInteraction,
    Crime,
    LifeEvent,
    Routine,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: MemoryEntryId,
    pub kind: MemoryKind,
    pub tick: u64,
    /// Bounded at 4 participants per ADR 0011.
    pub participants: Vec<AgentId>,
    pub location: crate::ids::LeafAreaId,
    /// In `[-1, 1]`.
    pub valence: f32,
    /// In `[0, 1]`. Decays over time; eviction key.
    pub importance: f32,
}

// ---------------------------------------------------------------------------
// (5) Relationships
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelField {
    Affinity,
    Trust,
    Familiarity,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RelationshipEdge {
    /// In `[-1, 1]`.
    pub affinity: f32,
    /// In `[0, 1]`.
    pub trust: f32,
    /// In `[0, 1]`.
    pub familiarity: f32,
    pub last_interaction_tick: u64,
}

// ---------------------------------------------------------------------------
// (6) Skills
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Skill {
    Social,
    Manual,
    Cognitive,
    Physical,
    Artistic,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Skills {
    pub social: f32,
    pub manual: f32,
    pub cognitive: f32,
    pub physical: f32,
    pub artistic: f32,
}

// ---------------------------------------------------------------------------
// (7) Money
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransactionKind {
    Wage,
    Purchase,
    Sale,
    Gift,
    Theft,
    Fine,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub tick: u64,
    pub kind: TransactionKind,
    /// Minor units (cents); per ADR 0011.
    pub amount: i64,
    pub counterparty: Option<AgentId>,
}

// ---------------------------------------------------------------------------
// (10) Health
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConditionKind {
    Illness,
    Injury,
    ChronicCondition,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HealthCondition {
    pub kind: ConditionKind,
    pub severity: f32,
    pub onset_tick: u64,
    pub expected_recovery_tick: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthState {
    pub conditions: Vec<HealthCondition>,
    /// Hits 0 → death (per ADR 0011).
    pub vitality: f32,
}

// ---------------------------------------------------------------------------
// (11) Crime
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Consequence {
    Fine { amount: i64, due_tick: u64 },
    CommunityService { hours: u32, due_tick: u64 },
    Probation { until_tick: u64 },
    Incarceration { until_tick: u64 },
}

// ---------------------------------------------------------------------------
// Cross-cutting: inventory
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    Food,
    WorkMaterial,
    StolenGood,
    Gift,
    Other,
}

bitflags::bitflags! {
    /// Per-item provenance flags (ADR 0011, "Inventory stacking").
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct ItemFlags: u8 {
        const STOLEN = 0b0000_0001;
        const GIFT = 0b0000_0010;
        const CONTRABAND = 0b0000_0100;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemMeta {
    pub origin: Option<AgentId>,
    pub origin_tick: u64,
    pub flags: ItemFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InventorySlot {
    pub item: ItemType,
    pub count: u16,
    pub metadata: Option<ItemMeta>,
}

// ---------------------------------------------------------------------------
// Top-level: Gecko
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gecko {
    // Identity
    pub id: AgentId,
    pub name: String,
    pub age: u16,
    pub gender: Gender,
    pub birth_tick: u64,
    pub alive: bool,

    pub appearance: Appearance,

    // Spatial
    pub current_leaf: crate::ids::LeafAreaId,
    pub position: Vec2,
    pub facing: Vec2,

    // Per-system state (numbering matches ADR 0010)
    pub needs: Needs,
    pub personality: Personality,
    pub mood: Mood,
    /// Bounded ring at 500 entries per ADR 0011 (importance × recency eviction).
    pub memory: Vec<MemoryEntry>,
    /// Sparse map; only non-trivial pairs stored.
    pub relationships: HashMap<AgentId, RelationshipEdge>,
    pub skills: Skills,
    /// Minor units (cents) per ADR 0011.
    pub money: i64,
    /// Bounded ring at 32 entries per ADR 0011.
    pub recent_transactions: Vec<Transaction>,
    pub residence: Option<HousingId>,
    pub household: Option<HouseholdId>,
    pub employment: Option<EmploymentId>,
    pub health: HealthState,
    pub criminal_record: Vec<CrimeIncidentId>,
    pub pending_consequences: Vec<Consequence>,

    // Cross-cutting
    /// Bounded at 8 slots per ADR 0011.
    pub inventory: Vec<InventorySlot>,

    // Decision runtime (per ADR 0004)
    pub current_action: Option<crate::decision::CommittedAction>,
    pub pending_interrupts: Vec<crate::decision::Interrupt>,
    /// Bounded ring at 16 entries per ADR 0011 (FIFO eviction).
    pub recent_actions: Vec<crate::decision::RecentActionEntry>,
    /// Bounded at 64 entries per ADR 0011.
    pub known_places: Vec<crate::ids::LeafAreaId>,

    // Determinism (per ADR 0008)
    pub rng: PrngState,
}

/// Reference target for action effects and predicates (per ADR 0011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NearbySelector {
    Random,
    Closest,
    HighestAffinity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetSpec {
    SelfAgent,
    OwnerOfObject,
    OtherAgent { id: AgentId },
    NearbyAgent { selector: NearbySelector },
}

```

The file ends after the closing brace of `TargetSpec`. (`ObjectId` is not imported here — the cross-module references in `decision/mod.rs` use `crate::ids::ObjectId` directly.)

- [ ] **Step 2: Add `bitflags` workspace dep**

`bitflags` is needed for `ItemFlags`. Add it to the workspace `[workspace.dependencies]` in `/Users/joseph/src/geckosim/Cargo.toml`:

```toml
bitflags = { version = "2", features = ["serde"] }
```

And to `crates/core/Cargo.toml` `[dependencies]`:

```toml
bitflags.workspace = true
```

- [ ] **Step 3: Add the module to `lib.rs`**

Append `pub mod agent;` to `crates/core/src/lib.rs`.

- [ ] **Step 4: Verify**

```bash
cd /Users/joseph/src/geckosim && cargo check -p gecko-sim-core
```

Expected: errors about `crate::decision::CommittedAction`, `crate::decision::Interrupt`, and `crate::decision::RecentActionEntry` not existing yet. **This is expected** — Task 8 introduces `decision`. Confirm those are the only errors and proceed.

If there are *other* errors (typos, missing imports, etc.) — fix those before continuing.

---

## Task 7: Smart objects, advertisements, predicates, effects

**Files:**
- Create: `crates/core/src/object/mod.rs`
- Modify: `crates/core/src/lib.rs`

**Goal:** Translate `SmartObject`, `ObjectType`, `Advertisement`, `Predicate`, `Effect`, `ScoreTemplate`, and `SituationalModifier` from ADR 0011.

- [ ] **Step 1: Write `crates/core/src/object/mod.rs`**

`/Users/joseph/src/geckosim/crates/core/src/object/mod.rs`:

```rust
//! Smart-object schema and the advertisement contract per ADR 0011.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::agent::{MoodDim, Need, Personality, RelField, Skill, TargetSpec};
use crate::ids::{AdvertisementId, ObjectId, ObjectTypeId, OwnerRef};
use crate::world::Vec2;

// ---------------------------------------------------------------------------
// Catalog and instance
// ---------------------------------------------------------------------------

/// Renderer mesh hint. Kept opaque at v0 — content authors fill these in.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct MeshId(pub u32);

/// Type-specific instance state. Keys are content-defined string identifiers;
/// values are typed.
pub type StateMap = HashMap<String, StateValue>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StateValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectType {
    pub id: ObjectTypeId,
    pub display_name: String,
    pub mesh_id: MeshId,
    pub default_state: StateMap,
    pub advertisements: Vec<Advertisement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartObject {
    pub id: ObjectId,
    pub type_id: ObjectTypeId,
    pub location: crate::ids::LeafAreaId,
    pub position: Vec2,
    pub owner: Option<OwnerRef>,
    pub state: StateMap,
}

// ---------------------------------------------------------------------------
// Advertisement contract
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InterruptClass {
    Always,
    NeedsThresholdOnly,
    Never,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Advertisement {
    pub id: AdvertisementId,
    pub display_name: String,
    pub preconditions: Vec<Predicate>,
    pub effects: Vec<Effect>,
    pub duration_ticks: u32,
    pub interrupt_class: InterruptClass,
    pub score_template: ScoreTemplate,
}

// ---------------------------------------------------------------------------
// Predicates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Op {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
    Ne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpatialReq {
    SameLeafArea,
    AdjacentArea,
    KnownPlace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TickRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MacroVar {
    // Stub variants — populated from ADR 0009 in a later pass.
    Weather,
    EmploymentRate,
    CrimeRate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MacroValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Predicate {
    AgentNeed(Need, Op, f32),
    AgentSkill(Skill, Op, f32),
    AgentInventory(crate::agent::ItemType, Op, u16),
    AgentRelationship(TargetSpec, RelField, Op, f32),
    ObjectState(String, Op, StateValue),
    MacroState(MacroVar, Op, MacroValue),
    Spatial(SpatialReq),
    TimeOfDay(TickRange),
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HealthChangeKind {
    AddCondition,
    RemoveCondition,
    AdjustVitality,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthChange {
    pub kind: HealthChangeKind,
    pub condition: Option<crate::agent::ConditionKind>,
    pub amount: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    // Stub — populated from ADR 0009's promoted-event taxonomy in a later pass.
    NeedCrisis,
    CrimeWitnessed,
    Death,
    Birth,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventPayload {
    None,
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    AgentNeedDelta(Need, f32),
    AgentMoodDelta(MoodDim, f32),
    AgentSkillDelta(Skill, f32),
    MoneyDelta(i64),
    InventoryDelta(crate::agent::ItemType, i32),
    MemoryGenerate {
        kind: crate::agent::MemoryKind,
        importance: f32,
        valence: f32,
        participants: TargetSpec,
    },
    RelationshipDelta(TargetSpec, RelField, f32),
    HealthConditionChange(HealthChange),
    PromotedEvent(EventType, EventPayload),
}

// (`PromotedEventId` is allocated by the sim during effect application, not
// stored on the effect itself — see ADR 0011 "Promoted-event emission".)

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SituationalModifier {
    MoodWeight {
        dim: MoodDim,
        weight: f32,
    },
    MacroVarWeight {
        var: MacroVar,
        weight: f32,
    },
    TimeOfDayWeight {
        peak_tick: u64,
        falloff: u32,
    },
    RelationshipWithTarget {
        field: RelField,
        weight: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreTemplate {
    pub need_weights: Vec<(Need, f32)>,
    pub personality_weights: Personality,
    pub situational_modifiers: Vec<SituationalModifier>,
}
```

- [ ] **Step 2: Add module to `lib.rs`**

Append `pub mod object;` to `crates/core/src/lib.rs`.

- [ ] **Step 3: Verify**

```bash
cd /Users/joseph/src/geckosim && cargo check -p gecko-sim-core
```

Expected: errors about `crate::decision::*` (still missing — Task 8 fixes). No new errors from object/.

---

## Task 8: Decision runtime

**Files:**
- Create: `crates/core/src/decision/mod.rs`
- Modify: `crates/core/src/lib.rs`

**Goal:** Translate the decision-runtime types from ADR 0011: `CommittedAction`, `Interrupt`, `RecentActionEntry`, plus their supporting enums.

- [ ] **Step 1: Write `crates/core/src/decision/mod.rs`**

`/Users/joseph/src/geckosim/crates/core/src/decision/mod.rs`:

```rust
//! Decision runtime types per ADR 0011 (action commitment & interruption).

use serde::{Deserialize, Serialize};

use crate::ids::{AdvertisementId, ObjectId, ObjectTypeId};
use crate::world::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    Walking,
    Performing,
    Completing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SelfActionKind {
    /// Agent-internal action with no smart object (e.g. wait, idle).
    Wait,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionRef {
    /// Action against a smart-object advertisement.
    Object {
        object: ObjectId,
        ad: AdvertisementId,
    },
    /// Self-action (no smart-object target).
    SelfAction(SelfActionKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CommittedAction {
    pub action: ActionRef,
    pub started_tick: u64,
    pub expected_end_tick: u64,
    pub phase: Phase,
    pub target_position: Option<Vec2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InterruptSource {
    NeedThreshold,
    MacroForcedAction,
    MacroPreconditionFailed,
    EnvironmentalEvent,
    AgentTargeted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InterruptPayload {
    None,
    NeedThreshold {
        need: crate::agent::Need,
    },
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Interrupt {
    pub source: InterruptSource,
    pub urgency: f32,
    pub payload: InterruptPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecentActionEntry {
    /// Template identity across instances per ADR 0011.
    pub ad_template: (ObjectTypeId, AdvertisementId),
    pub completed_tick: u64,
}
```

- [ ] **Step 2: Add module to `lib.rs`**

Append `pub mod decision;` to `crates/core/src/lib.rs`.

- [ ] **Step 3: Verify**

```bash
cd /Users/joseph/src/geckosim && cargo check -p gecko-sim-core
```

Expected: clean. All cross-module references should now resolve.

---

## Task 9: Empty / stub modules — `macro_`, `events`, `systems`, `save`

**Files:**
- Create: `crates/core/src/macro_/mod.rs`
- Create: `crates/core/src/events/mod.rs`
- Create: `crates/core/src/systems/mod.rs`
- Create: `crates/core/src/save/mod.rs`
- Modify: `crates/core/src/lib.rs`

**Goal:** Reserve the module slots from ADR 0012 with intent-bearing comments. No real code yet.

- [ ] **Step 1: Write `crates/core/src/macro_/mod.rs`**

`/Users/joseph/src/geckosim/crates/core/src/macro_/mod.rs`:

```rust
//! Macro state and macro-tick logic per ADR 0009.
//!
//! At scaffold pass, only the cross-cutting `MacroVar` enum lives elsewhere
//! (in `object` module, used by `Predicate::MacroState`). The macro tick
//! loop, demographics, weather, and policy state land here in a later pass.

// Empty — see ADR 0009.
```

- [ ] **Step 2: Write `crates/core/src/events/mod.rs`**

`/Users/joseph/src/geckosim/crates/core/src/events/mod.rs`:

```rust
//! Promoted event channel per ADR 0009.
//!
//! The promoted-event ring, cursor, and cross-tick replay are all deferred to
//! a later pass. The wire/serializable shape of an emitted event is sketched
//! here when the host wires the WS server.

// Empty — see ADR 0009 ("Promoted events").
```

- [ ] **Step 3: Write `crates/core/src/systems/mod.rs`**

`/Users/joseph/src/geckosim/crates/core/src/systems/mod.rs`:

```rust
//! ECS systems per ADR 0010 / 0012.
//!
//! Each v0 system from ADR 0010 will land as its own submodule:
//!   - `needs`         (1) need decay
//!   - `personality`   (2) personality (read-only)
//!   - `mood`          (3) mood update
//!   - `memory`        (4) memory ring & decay
//!   - `relationships` (5) relationship updates
//!   - `skills`        (6) skill gain
//!   - `money`         (7) wages, transactions
//!   - `housing`       (8) residence assignment
//!   - `employment`    (9) job scheduling
//!   - `health`        (10) condition + vitality
//!   - `crime`         (11) crime + consequences
//!
//! Systems are added in a later pass alongside the live `Sim` API.

// Empty — see ADR 0010 for the v0 system list.
```

- [ ] **Step 4: Write `crates/core/src/save/mod.rs`**

`/Users/joseph/src/geckosim/crates/core/src/save/mod.rs`:

```rust
//! Save / load per ADR 0012.
//!
//! `SaveData` (postcard for production, JSON for debug) is defined in a later
//! pass once the live `Sim` API exists.

// Empty.
```

- [ ] **Step 5: Add modules to `lib.rs`**

Append to `crates/core/src/lib.rs`:

```rust
pub mod events;
pub mod macro_;
pub mod save;
pub mod systems;
```

- [ ] **Step 6: Verify**

```bash
cd /Users/joseph/src/geckosim && cargo check -p gecko-sim-core
```

Expected: clean.

---

## Task 10: Re-exports + final `lib.rs` shape

**Files:**
- Modify: `crates/core/src/lib.rs`

**Goal:** Curate the crate-level public surface. Re-export the most-used types so consumers can `use gecko_sim_core::AgentId` instead of `gecko_sim_core::ids::AgentId`.

- [ ] **Step 1: Replace `crates/core/src/lib.rs`**

Final contents:

```rust
//! Gecko-sim core: schema types and (later) the ECS-based simulation engine.
//!
//! At this scaffold pass, only the v0 schema from ADR 0011 is implemented.
//! The live `Sim` API, ECS components, and systems land in later passes.

pub mod agent;
pub mod decision;
pub mod events;
pub mod ids;
pub mod macro_;
pub mod object;
pub mod rng;
pub mod save;
pub mod systems;
pub mod time;
pub mod world;

// Convenience re-exports of the most-used public types.
pub use ids::{
    AccessoryId, AdvertisementId, AgentId, BuildingId, BusinessId, CrimeIncidentId, EmploymentId,
    HouseholdId, HousingId, LeafAreaId, MemoryEntryId, ObjectId, ObjectTypeId, OwnerRef,
    PromotedEventId,
};
pub use rng::PrngState;
pub use time::Tick;
pub use world::{Color, Vec2};
```

- [ ] **Step 2: Verify**

```bash
cd /Users/joseph/src/geckosim && cargo check -p gecko-sim-core
```

Expected: clean.

---

## Task 11: Smoke test for `gecko-sim-core`

**Files:**
- Create: `crates/core/tests/smoke.rs`

**Goal:** Confirm the public surface exposes the headline types.

- [ ] **Step 1: Write `crates/core/tests/smoke.rs`**

`/Users/joseph/src/geckosim/crates/core/tests/smoke.rs`:

```rust
//! Smoke test: confirm the headline schema types compile and are reachable
//! from outside the crate via the public surface.

use gecko_sim_core::agent::{Mood, Needs, Personality};
use gecko_sim_core::ids::{AgentId, OwnerRef};
use gecko_sim_core::object::{Predicate, SmartObject};
use gecko_sim_core::{Color, PrngState, Tick, Vec2};

#[test]
fn ids_construct_and_round_trip() {
    let a = AgentId::new(42);
    assert_eq!(a.raw(), 42);
}

#[test]
fn primitives_construct() {
    let _ = Color::new(255, 128, 0);
    let _ = Vec2::new(1.0, 2.0);
    let _ = Tick::new(0);
    let _ = PrngState::from_seed(0xDEAD_BEEF);
}

#[test]
fn schema_types_are_reachable() {
    // We only need to name the types — instantiation requires populating
    // ~30 fields, which is the live-sim pass's job, not the scaffold's.
    let _ = std::mem::size_of::<Needs>();
    let _ = std::mem::size_of::<Personality>();
    let _ = std::mem::size_of::<Mood>();
    let _ = std::mem::size_of::<SmartObject>();
    let _ = std::mem::size_of::<Predicate>();
    let _ = std::mem::size_of::<OwnerRef>();
}
```

- [ ] **Step 2: Run the test**

```bash
cd /Users/joseph/src/geckosim && cargo test -p gecko-sim-core
```

Expected: `test result: ok. 3 passed; 0 failed`.

---

## Task 12: Smoke tests for `content` and `protocol`

**Files:**
- Create: `crates/content/tests/smoke.rs`
- Create: `crates/protocol/tests/smoke.rs`

**Goal:** Each library crate has a smoke test that proves the crate compiles in test mode and that its dep on `gecko-sim-core` resolves.

- [ ] **Step 1: Write `crates/content/tests/smoke.rs`**

`/Users/joseph/src/geckosim/crates/content/tests/smoke.rs`:

```rust
//! Smoke test for `gecko-sim-content`. The crate is intentionally empty at
//! the scaffold pass; this test just confirms it compiles and links to
//! `gecko-sim-core`.

use gecko_sim_core::AgentId;

#[test]
fn dep_chain_resolves() {
    let _ = AgentId::new(0);
}
```

- [ ] **Step 2: Write `crates/protocol/tests/smoke.rs`**

`/Users/joseph/src/geckosim/crates/protocol/tests/smoke.rs`:

```rust
//! Smoke test for `gecko-sim-protocol`. Empty crate body at the scaffold
//! pass; the smoke test confirms the dep chain compiles.

use gecko_sim_core::AgentId;

#[test]
fn dep_chain_resolves() {
    let _ = AgentId::new(0);
}
```

- [ ] **Step 3: Verify**

```bash
cd /Users/joseph/src/geckosim && cargo test --workspace
```

Expected: 5 tests pass (3 in core, 1 in content, 1 in protocol). Host crate has no tests.

---

## Task 13: Confirm host main is final-shape-correct

**Files:**
- Inspect: `crates/host/src/main.rs` (no edit expected)

**Goal:** Re-read the host main written in Task 1 and confirm it matches the spec exactly. No code change unless drift.

- [ ] **Step 1: Read the file**

Verify the contents are exactly:

```rust
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
```

If different (e.g. an extra placeholder body), update to match.

- [ ] **Step 2: Run it**

```bash
cd /Users/joseph/src/geckosim && cargo run -p gecko-sim-host
```

Expected: a single tracing line ending in `gecko-sim host v0.1.0`, then exit 0.

---

## Task 14: Definition-of-done verification

**Files:** none (verification only).

**Goal:** Run every check from the spec's Definition of Done. Stop and fix anything that fails.

- [ ] **Step 1: `cargo build --workspace`**

```bash
cd /Users/joseph/src/geckosim && cargo build --workspace
```

Expected: clean build, no warnings.

- [ ] **Step 2: `cargo test --workspace`**

```bash
cd /Users/joseph/src/geckosim && cargo test --workspace
```

Expected: 5 tests pass.

- [ ] **Step 3: `cargo clippy --workspace --all-targets -- -D warnings`**

```bash
cd /Users/joseph/src/geckosim && cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean. `clippy::pedantic` will likely complain about minor style issues — fix them inline (rename a `_` import, add `#[must_use]`, etc.) until clean.

If a pedantic lint demands a substantive change to the schema (e.g. struct-field naming) — `#[allow(clippy::lint_name)]` it on the offending item with a short comment, rather than diverging from ADR 0011. Schema fidelity wins.

- [ ] **Step 4: `cargo fmt --check`**

```bash
cd /Users/joseph/src/geckosim && cargo fmt --check
```

Expected: clean. If not, run `cargo fmt --all` and re-check.

- [ ] **Step 5: `cargo run -p gecko-sim-host`**

```bash
cd /Users/joseph/src/geckosim && cargo run -p gecko-sim-host
```

Expected: tracing line + exit 0.

- [ ] **Step 6: `jj st` final review**

```bash
jj st
jj diff --stat
```

Expected:
- Working copy `@` is the change opened in Task 1.
- Description: `Scaffold workspace and v0 schema types from 0011/0012`.
- File list matches the file map at the top of this plan (excluding the spec/plan docs themselves, which were committed earlier).

If any file is missing or extra, fix before the next step.

- [ ] **Step 7: Hand off**

Notify the user: scaffold complete, all DoD checks pass, single jj change ready for review. The change is mutable — they can amend the description, squash, or push it as they see fit.

---

## Self-review

Spec coverage walk:

- **Filesystem layout** (spec §"Repository layout") → Task 1 covers root files; Tasks 2–9 cover `core/src/`; Tasks 11–12 cover smoke tests; Task 1 covers `content/README.md` and `apps/web/README.md`. ✓
- **Workspace manifest** (spec §"Workspace manifest") → Task 1 step 2 (root `Cargo.toml` with shared deps and lints). ✓
- **Per-crate dependencies** (spec table) → Task 1 steps 5/7/9/11 (per-crate Cargo.toml files). Note: `bitflags` got added in Task 6 step 2 — the spec table doesn't list it; this is a small addition justified by `ItemFlags`. (Spec is authoritative; this is a faithful interpretation, not a deviation. Plan documents the addition.) ✓
- **Type translation** (spec §"Type translation strategy") →
  - Identifiers → Task 2 ✓
  - Math/color → Task 3 ✓
  - Bounded collections aliased to `Vec`/`HashMap` → Tasks 6, 7 (used throughout) ✓
  - RNG → Task 4 ✓
  - Top-level structures monolithic → Task 6 (`Gecko`), Task 7 (`SmartObject`, etc.) ✓
  - Default derive set → applied throughout Tasks 2–8 ✓
  - No `ts-rs` derives → none added; deferred ✓
- **`host/src/main.rs`** (spec §"host/src/main.rs") → Task 1 step 12 + Task 13 verification. ✓
- **Tooling** (spec §"Tooling") → `rustfmt.toml` Task 1 step 3; `.gitignore` Task 1 step 4. ✓
- **Tests** (spec §"Tests") → Tasks 11, 12 (smoke tests per library crate; host has none). ✓
- **Definition of done** (spec §"Definition of done") → Task 14 runs every check. ✓

Placeholder scan: no `TBD` / `TODO` / "implement later" / "fill in details" / "similar to Task N" patterns anywhere in the steps. Stub modules in Task 9 are intentional and contain only doc comments — they are not "fill in later" placeholders for *this* pass; they are explicit deferrals tracked in the spec's "Non-goals" / "Explicitly deferred" sections.

Type consistency: `AgentId::new(...)` and `AgentId::raw(...)` are defined in Task 2 (via the `id_newtype!` macro) and used in Tasks 11/12 — match. `PrngState::from_seed` defined in Task 4, used in Task 11 — match. `Color::new` defined in Task 3, used in Task 11 — match. `Tick::new` defined in Task 5, used in Task 11 — match. The cross-module references in `Gecko` (`crate::decision::CommittedAction`, `crate::decision::Interrupt`, `crate::decision::RecentActionEntry`) are introduced in Task 8 — Task 6 step 4 explicitly notes the expected compile error and directs the implementer not to debug it.
