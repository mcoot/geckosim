# v0 scaffolding pass — Rust workspace + 0011 type stubs

- **Date:** 2026-04-26
- **Status:** Approved
- **Scope:** First implementation pass on the gecko sim. Sets up the Rust workspace and translates the schema in ADR 0011 into compilable Rust types.

## Goal

Land a 4-crate Rust workspace per ADR 0012, with the full v0 schema from ADR 0011 translated into real Rust types, building cleanly and lint-clean. End state is a chassis that the next pass (live ECS, RON loaders, WS server) can drop into without moving files.

Frontend (`apps/web/`) is **not** scaffolded in this pass — it gets a placeholder `README.md` only. Type generation (`ts-rs`) and the wire layer (`protocol` crate's WS types) are deferred to the pass that wires the frontend.

## Non-goals

- No live ECS / `Sim` API. Types only — no `bevy_ecs::World` is built. `Sim::new`, `tick`, `snapshot`, etc. arrive in a later pass.
- No RON loading or `content/` files. The `content` crate compiles but exports nothing concrete yet.
- No WebSocket server, sample loop, or tick scheduler. `host` is a hello-world binary.
- No `ts-rs` derives. Adding them belongs with the frontend pass when there's a real emit target.
- No CI workflow file. Add when the user asks.
- No `rust-toolchain.toml`. Use whatever's installed; pin only when a feature requires it.

## Repository layout after this pass

```
gecko-sim/                          ← repo root (already exists)
├── Cargo.toml                      workspace + shared deps + lints
├── rustfmt.toml
├── .gitignore                      (target/, .DS_Store)
├── crates/
│   ├── core/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs              re-exports
│   │   ├── src/ids.rs              all *Id newtypes
│   │   ├── src/world/mod.rs        Vec2 alias, Color, LeafArea sketch
│   │   ├── src/agent/mod.rs        Gecko + supporting types
│   │   ├── src/object/mod.rs       SmartObject, ObjectType, Advertisement
│   │   ├── src/decision/mod.rs     CommittedAction, Interrupt, etc.
│   │   ├── src/macro_/mod.rs       MacroVar enum stub
│   │   ├── src/systems/mod.rs      empty (one comment per system from 0010)
│   │   ├── src/events/mod.rs       PromotedEvent type stub
│   │   ├── src/rng/mod.rs          PrngState newtype around Pcg64Mcg
│   │   ├── src/time/mod.rs         Tick newtype, sim-time consts
│   │   ├── src/save/mod.rs         empty
│   │   └── tests/smoke.rs
│   ├── content/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs              empty pub use; re-exports core schema later
│   │   └── tests/smoke.rs
│   ├── protocol/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs              empty pub use; wire types added later
│   │   └── tests/smoke.rs
│   └── host/
│       ├── Cargo.toml
│       └── src/main.rs             tracing init + version log + exit
├── content/README.md               placeholder; future RON files land here
├── apps/web/README.md              placeholder; Next.js scaffold deferred
└── docs/                           unchanged
```

The submodule structure under `core/src/` mirrors ADR 0012 verbatim — empty modules included — so later passes drop code into the right spot without restructuring.

## Workspace manifest

`Cargo.toml` at repo root:

- `[workspace]` with `members = ["crates/*"]`.
- `[workspace.package]` shared metadata: `version = "0.1.0"`, `edition = "2021"`, `license = "MIT"` (matches the existing `LICENSE`).
- `[workspace.dependencies]` shared:
  - `serde = { version = "1", features = ["derive"] }`
  - `thiserror = "2"`
  - `bevy_ecs = "0.16"`
  - `tracing = "0.1"`
  - `glam = { version = "0.30", features = ["serde"] }`
  - `rand = "0.9"`
  - `rand_pcg = "0.9"`
- `[workspace.lints.rust]`: `unsafe_code = "forbid"`, `warnings = "deny"`.
- `[workspace.lints.clippy]`: `pedantic = { level = "warn", priority = -1 }`, with explicit `module_name_repetitions = "allow"` and `missing_errors_doc = "allow"`. Curated, not the firehose — v0 code shouldn't fight clippy::pedantic on every line.

Each crate inherits via `[lints] workspace = true`.

Versions are best-known-good as of 2026-04-26; the wiring is not version-sensitive — bumping later is mechanical.

## Per-crate dependencies

| Crate | Cargo package name | Depends on | Why |
|---|---|---|---|
| `crates/core/` | `gecko-sim-core` | serde, thiserror, bevy_ecs, glam, rand, rand_pcg | schema + ECS dep imported (not yet used) so we hit any compile pain now |
| `crates/content/` | `gecko-sim-content` | gecko-sim-core, serde, thiserror, ron | RON loader will land here in a later pass |
| `crates/protocol/` | `gecko-sim-protocol` | gecko-sim-core, serde | wire types will live here; ts-rs deferred |
| `crates/host/` | `gecko-sim-host` | the three above, tracing, tracing-subscriber, anyhow | binary entry point |

Package names are prefixed `gecko-sim-` to avoid colliding with anything in `std` (notably `core`) or the broader crates.io namespace. Library crate names at `use`-site become underscored: `gecko_sim_core`, etc.

`bevy_ecs` is added to `gecko-sim-core` even though no `World` is built yet. Importing it now surfaces any version-resolution issues at scaffold time, not later.

## Type translation strategy

Mechanical translation of ADR 0011's pseudo-Rust into real Rust. Specific decisions:

### Identifiers (`core/src/ids.rs`)

All `*Id` newtypes from 0011: `AgentId`, `ObjectId`, `ObjectTypeId`, `BuildingId`, `LeafAreaId`, `HousingId`, `EmploymentId`, `HouseholdId`, `BusinessId`, `CrimeIncidentId`, `MemoryEntryId`, `AccessoryId`, `AdvertisementId`, `PromotedEventId`. Each is a `pub struct FooId(pub u64);` with `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize` derives.

`OwnerRef` is a sum-type enum over `AgentId | HouseholdId | BusinessId`.

### Math and color (`core/src/world/mod.rs`)

- `Vec2` — type alias `pub use glam::Vec2;` (no wrapper).
- `Color` — `pub struct Color { r: u8, g: u8, b: u8 }` with derives. No alpha, no HDR at v0.

### Bounded collections

ADR 0011 mentions `BoundedRing<T, N>`, `BoundedVec<T, N>`, `SparseMap<K, V>`. For v0 these are aliased to `Vec<T>` and `std::collections::HashMap<K, V>`. A doc comment on each alias notes the bound from 0011 (e.g. memory ring cap = 500, inventory cap = 8). Real bounded types deferred.

### RNG (`core/src/rng/mod.rs`)

`pub struct PrngState(pub rand_pcg::Pcg64Mcg);` with `Debug, Clone, Serialize, Deserialize`. Deterministic, small state, fast — appropriate for the per-agent seeded sub-stream pattern from ADR 0008.

### Top-level structures

`Gecko` lives as a single monolithic struct in `core/src/agent/mod.rs`, faithful to 0011. ECS sharding into separate components (`Needs`, `Personality`, `Mood`, …) happens in the next pass when `Sim` lands. Until then, this monolithic shape is also useful for the snapshot / wire / save shapes that 0013 references.

`SmartObject`, `ObjectType`, `Advertisement` live in `core/src/object/mod.rs`. `Predicate` and `Effect` enums get all variants from 0011 — the user explicitly chose option C (full schema) so no variants are skipped.

`CommittedAction`, `Interrupt`, `RecentActionEntry` live in `core/src/decision/mod.rs`. `MacroVar` placeholder enum lives in `core/src/macro_/mod.rs`.

### Derives

Default derive set on every schema struct/enum: `Debug, Clone, Serialize, Deserialize`. Add `PartialEq, Eq, Hash` where the type is naturally comparable (IDs, simple enums). `Copy` only on small POD-like types (IDs, `Color`, dimensionless enums).

`#[derive(TS)]` is deliberately **not** added in this pass.

## `host/src/main.rs`

```rust
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("gecko-sim host v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
```

Proves the binary builds, links `tracing` correctly, and `cargo run -p host` runs.

## Tooling

- `rustfmt.toml`: `edition = "2021"`, `max_width = 100`. No other overrides — match Rust community defaults.
- `.gitignore`: adds `/target` and `.DS_Store`. Repo currently has no `.gitignore`.

## Tests

One smoke test per library crate (`core/tests/smoke.rs`, `content/tests/smoke.rs`, `protocol/tests/smoke.rs`):

```rust
// core/tests/smoke.rs
use gecko_sim_core::ids::AgentId;

#[test]
fn ids_construct() {
    let _ = AgentId(0);
}
```

The point is confirming the crate's public surface compiles and exports cleanly — not unit-testing logic that doesn't exist yet.

`host` has no tests in this pass; its `main` runs at `cargo run -p host`.

## Definition of done

The pass is complete when, from a clean checkout:

- `cargo build --workspace` succeeds.
- `cargo test --workspace` passes (smoke tests only).
- `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- `cargo run -p gecko-sim-host` prints `gecko-sim host v0.1.0` and exits 0.
- One `jj` commit with description `Scaffold workspace and v0 schema types from 0011/0012` (or close), atomic — no follow-up cleanup required.

## Trace to ADRs

- **0011 (schema):** every type translated; bounded collections temporarily aliased; ts-rs deferred.
- **0012 (crate architecture):** 4-crate layout, module structure under `core/src/`, dep direction (`core ← content`, `core ← protocol`, all ← `host`), shared lints, error library choice (`thiserror` in libs, `anyhow` in `host`), tracing dep wired.
- **0013 (frontend transport):** wire types not added in this pass — `protocol` crate exists but is empty pending the WS scaffolding pass.

## What this pass enables next

The next pass starts with the workspace and schema in place and adds the live runtime: build a `bevy_ecs::World` inside `core`, define ECS components by sharding the monolithic `Gecko`, write the `Sim` API surface from 0012, add the first system from 0010 (likely needs decay — smallest), and run a tick. Once that works, follow with RON loaders and the WS server in subsequent passes.
