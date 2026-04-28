# Spatial pass — hierarchical world, agent positions, Phase::Walking

- **Date:** 2026-04-28
- **Status:** Draft
- **Scope:** Ninth implementation pass. Lands the spatial substrate from ADR 0007: a hierarchical world graph (district → building → floor → leaf area + outdoor zones), `Position` / `Facing` ECS components on agents, and a real `Phase::Walking` step in the decision runtime. Grows the wire so Three.js has the data it needs (per-tick agent + smart-object positions; once-per-init world layout). Frontend rendering itself (the Three.js scene) is its own later pass — this pass exposes the inputs.
- **Predecessors:**
  - [`2026-04-28-decision-runtime-v0-design.md`](2026-04-28-decision-runtime-v0-design.md) — wired `decide` and `execute` into the schedule. Today every committed action goes straight to `Phase::Performing`; `Phase::Walking` exists structurally but is unused.
  - [`2026-04-28-personality-system-design.md`](2026-04-28-personality-system-design.md) — established the snapshot-grows + ts-rs-regen + frontend-fixture flow this pass reuses.
  - [`2026-04-27-ws-transport-v0-design.md`](2026-04-27-ws-transport-v0-design.md) — `Init` carries one-shot data; `Snapshot` carries per-tick data. This pass adds a `world` field to `Init` and grows `Snapshot` with positions + objects.

## Goal

End state:

1. `crates/core/src/world/` grows the ADR 0007 type tree: `Rect2`, `LeafKind`, `OutdoorZoneKind`, `LeafArea`, `District`, `Building`, `Floor`, plus a `WorldGraph` resource holding flat hashmaps keyed by ID. `World::seed_v0()` returns a tiny working seed: 1 district, 1 plaza (outdoor), 1 forecourt, 1 building (1 floor, 1 living-room leaf).
2. `Vec2` becomes a wire-friendly `{ x: f32, y: f32 }` struct exported via ts-rs. The `glam::Vec2` re-export is removed; the few local math sites use plain f32 arithmetic.
3. `Position { leaf, pos }` and `Facing { dir }` land as ECS components (lazy-shard from `Gecko::current_leaf` / `Gecko::position` / `Gecko::facing`). `Sim::spawn_test_agent_with_needs` places agents in `WorldGraph::default_spawn_leaf()` at a deterministic position.
4. `Sim::new` inserts the `WorldGraph` resource (built from `World::seed_v0()`) and the host's seed-instance spawn places the chair in the living-room leaf and the fridge there too — same leaf as the agents at v0, so all advertisements remain reachable without cross-leaf pathfinding.
5. `CommittedAction` grows `perform_duration_ticks: u32` and `expected_end_tick: Option<u64>`. `decide` picks `Phase::Walking` for object-targeted actions whose target position is non-zero distance from the agent (i.e. all chair / fridge picks); self-actions (`Idle`, `Wait`) stay on `Phase::Performing` with their fixed end tick.
6. `systems::movement::walk` advances each `Walking` agent's `Position.pos` toward `target_position` at `WALK_SPEED_M_PER_TICK = 80.0` m/min. On arrival the agent transitions to `Phase::Performing`, sets `started_tick = current_tick`, and computes `expected_end_tick = current_tick + perform_duration_ticks`. `Facing.dir` updates to the unit-vector direction of motion at each step (or stays put when stationary).
7. `Predicate::Spatial(SameLeafArea)` actually compares `agent.leaf == object.leaf`. Other `SpatialReq` variants stay placeholders (return `true`) with a TODO. The `EvalContext` grows an `agent_leaf: LeafAreaId` field; `decide` passes the agent's `Position.leaf`.
8. `AgentSnapshot` grows `leaf, pos, facing, action_phase`. New `ObjectSnapshot { id, type_id, leaf, pos }` lands; `Snapshot` grows `objects: Vec<ObjectSnapshot>`. New `WorldLayout { districts, buildings, floors, leaves }` lands as the lossy renderer projection of the world graph; `ServerMessage::Init` grows a `world` field carrying it. ts-rs regenerates `Vec2.ts`, `Rect2.ts`, `LeafKind.ts`, `OutdoorZoneKind.ts`, `LeafArea.ts`, `District.ts`, `Building.ts`, `Floor.ts`, `WorldLayout.ts`, `ObjectSnapshot.ts`, `Phase.ts`, and updates `AgentSnapshot.ts` / `Snapshot.ts` / `ServerMessage.ts`.
9. Frontend `SimState` grows a `world: WorldLayout | null` field set on `Init`. `<AgentList>` adds a "Where" column showing `leaf:(x, y)`. The Three.js scene itself is deferred — this pass plumbs the data so the next pass can mount a renderer without further sim-side changes.

This is the smallest pass that flips spatial-from-stub to spatial-real while keeping cross-leaf pathfinding deferred. After it lands, every renderable thing the agent does is observable on the wire as a position + phase + leaf.

## Non-goals (deferred)

- **Cross-leaf pathfinding (A\* on the building/district graph).** ADR 0007 calls for hierarchical A*, but at v0 the seed world has agents and all reachable smart objects in the same leaf area. The leaf-adjacency edges are populated in `WorldGraph` so a future pass plugs in routing without touching the data shape.
- **Smart-object discovery scoping** ("walk leaf → building → district" per ADR 0007). The decide loop still sees every smart object in the world. Adding the scope filter is mechanical once cross-leaf actions exist.
- **`Predicate::Spatial(AdjacentArea | KnownPlace)`.** Both still return `true`. The first needs working leaf adjacency lookups in the predicate evaluator (data is there, hookup is small but pulled into the cross-leaf pass). The second needs the agent's `known_places` field as an ECS component, which is a separate system.
- **Renderer interpolation between samples** (ADR 0008 / 0013). The renderer is for the next pass; interpolation is a polish concern beyond that.
- **Three.js scene itself.** This pass's frontend visible change is one column in `<AgentList>`. Mounting an actual `<canvas>` with `OrbitControls`, leaf-area tile meshes, and animated agent dots is the next pass.
- **Vehicles, transit graph, district adjacency.** ADR 0007 reserves these explicitly.
- **`Position` / `Facing` save format.** Saves don't exist yet (per ADR 0010); when they land, these fields ship in the snapshot serialization that's already wire-frozen by this pass.
- **Continuous facing animation.** `Facing.dir` updates step-wise on each tick of motion. Sub-tick smoothing is a renderer concern.
- **More than one floor per building, more than one room per floor, more than one building per district.** All structurally permitted; seed world deliberately tiny.
- **Authored world content (RON files).** `World::seed_v0()` is hard-coded Rust. Loading districts/buildings/leaves from RON is a content pass.

## Architecture

### `crate::world` type tree (ADR 0007)

```rust
// crates/core/src/world/types.rs (new submodule)

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self { Self { x, y } }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))]
pub struct Rect2 {
    pub min: Vec2,
    pub max: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))]
pub enum OutdoorZoneKind {
    Plaza,
    Forecourt,
    StreetSegment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))]
pub enum LeafKind {
    Room { building: BuildingId, floor: FloorId },
    OutdoorZone(OutdoorZoneKind),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))]
pub struct LeafArea {
    pub id: LeafAreaId,
    pub display_name: String,
    pub kind: LeafKind,
    pub bbox: Rect2,
    pub adjacency: Vec<LeafAreaId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))]
pub struct District {
    pub id: DistrictId,
    pub display_name: String,
    pub bbox: Rect2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))]
pub struct Building {
    pub id: BuildingId,
    pub display_name: String,
    pub district: DistrictId,
    pub footprint: Rect2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))]
pub struct Floor {
    pub id: FloorId,
    pub building: BuildingId,
    pub level: i16,
}
```

`DistrictId` (already in `ids.rs`) keeps its existing newtype. `FloorId` is **new** — added as one more `id_newtype!(FloorId)` line.

### `WorldGraph` resource

```rust
// crates/core/src/world/graph.rs (new submodule)

#[derive(bevy_ecs::prelude::Resource, Debug, Clone, PartialEq)]
pub struct WorldGraph {
    pub districts: HashMap<DistrictId, District>,
    pub buildings: HashMap<BuildingId, Building>,
    pub floors:    HashMap<FloorId, Floor>,
    pub leaves:    HashMap<LeafAreaId, LeafArea>,
    pub default_spawn_leaf: LeafAreaId,   // where v0's spawn helpers drop new agents
}

impl WorldGraph {
    /// Build the v0 seed world: 1 district, 1 plaza, 1 forecourt,
    /// 1 building (1 floor, 1 living-room leaf). Adjacency:
    /// plaza ↔ forecourt; forecourt ↔ living-room.
    /// `default_spawn_leaf` = living-room (so all v0 agents and smart
    /// objects share one leaf, no walking across leaves needed yet).
    #[must_use]
    pub fn seed_v0() -> Self { /* ... */ }

    #[must_use]
    pub fn leaf(&self, id: LeafAreaId) -> Option<&LeafArea> { self.leaves.get(&id) }

    #[must_use]
    pub fn are_adjacent(&self, a: LeafAreaId, b: LeafAreaId) -> bool {
        self.leaves.get(&a).is_some_and(|l| l.adjacency.contains(&b))
    }
}
```

`crate::world` ends up with three submodules: `types` (the data shapes above), `graph` (`WorldGraph` + `seed_v0`), and the existing `Color`. The current `world/mod.rs` re-exports them all and **drops** `pub use glam::Vec2;`.

### `Vec2` migration

`glam::Vec2` was re-exported as `crate::world::Vec2` and used in:
- `crate::world::Vec2` (the re-export itself) — replaced by the local struct above.
- `agent::Gecko::position` / `facing` — purely schema (no live use yet); type swap is mechanical.
- `decision::CommittedAction::target_position` — already typed against `Vec2`, swap is mechanical.
- `object::SmartObject::position` — already typed against `Vec2`, swap is mechanical.
- `sim::Sim::spawn_test_object` and `sim::Sim::spawn_one_of_each_object_type` argument lists — type swap.
- Test files (`tests/decision.rs`, `tests/smoke.rs`, plus `decide.rs` and `execute.rs` test modules).

Math the spatial pass needs (vector subtraction, length, scale toward target) is implemented inline:

```rust
fn step_toward(from: Vec2, to: Vec2, max_dist: f32) -> (Vec2, Vec2) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let dist_sq = dx * dx + dy * dy;
    if dist_sq <= max_dist * max_dist {
        // Arrive: snap to target, facing unchanged.
        return (to, Vec2::ZERO);
    }
    let dist = dist_sq.sqrt();
    let inv = 1.0 / dist;
    let dir = Vec2 { x: dx * inv, y: dy * inv };
    let next = Vec2 {
        x: from.x + dir.x * max_dist,
        y: from.y + dir.y * max_dist,
    };
    (next, dir)
}
```

Returns `(next_pos, new_facing)`. When the call arrived, `new_facing` is `ZERO` to signal "don't update facing" (caller leaves the existing `Facing.dir` alone).

`glam` stays in the workspace `Cargo.toml` for now (no other code depends on it after this pass; left alone to avoid an unrelated dep churn).

### Position / Facing components

```rust
// crates/core/src/agent/spatial.rs (new submodule, re-exported from agent::mod)

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))]
pub struct Position {
    pub leaf: LeafAreaId,
    pub pos: Vec2,
}

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))]
pub struct Facing {
    pub dir: Vec2,
}

impl Default for Facing {
    fn default() -> Self { Self { dir: Vec2 { x: 1.0, y: 0.0 } } }   // facing +x by default
}
```

Lazy-shard convention: the schema struct `Gecko` already has `current_leaf`, `position`, `facing` fields. The ECS component `Position` collapses `current_leaf + position` into one component because they update together (an agent crosses a leaf boundary by changing both). `Facing` stays separate because mood / interaction systems may update it without moving.

### `Phase` becomes a wire type

```rust
// crates/core/src/decision/mod.rs (already exists)

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "../../apps/web/src/types/sim/"))]
pub enum Phase {
    Walking,
    Performing,
    Completing,
}
```

(The enum already exists; the pass adds the `cfg_attr` block.)

### `CommittedAction` shape change

```rust
pub struct CommittedAction {
    pub action: ActionRef,
    pub started_tick: u64,
    pub expected_end_tick: Option<u64>,    // ← was u64; None during Walking
    pub phase: Phase,
    pub target_position: Option<Vec2>,
    pub perform_duration_ticks: u32,       // ← new; remembered through Walking
}
```

Rationale: `expected_end_tick` was set at decide time off the action's duration. With Walking inserted, the perform end-tick is unknown until the agent arrives (walking distance varies). `Option<u64>` makes that explicit. `perform_duration_ticks` carries the perform duration through the walking phase so the movement system can compute `expected_end_tick` at arrival.

For self-actions (`Idle`, `Wait`) — no walking — `decide` sets `phase = Performing`, `expected_end_tick = Some(started_tick + perform_duration_ticks)` immediately.

### `decide` system updates

```rust
// in pick_next_action, after picking the (object, ad, duration) tuple:

let target_obj = objects.iter().find(|o| o.id == object_id).unwrap();   // unwrap-safe: just iterated
let target_pos = target_obj.position;
let agent_pos = position.pos;
let same_spot = approx_eq(target_pos, agent_pos);

let (phase, end_tick) = if same_spot {
    (Phase::Performing, Some(current_tick + u64::from(duration_ticks)))
} else {
    (Phase::Walking, None)
};

CommittedAction {
    action: ActionRef::Object { object: object_id, ad: ad_id },
    started_tick: current_tick,
    expected_end_tick: end_tick,
    phase,
    target_position: Some(target_pos),
    perform_duration_ticks: duration_ticks,
}
```

Self-action branch keeps `Phase::Performing`, sets `target_position: None`, and computes `expected_end_tick = Some(current_tick + IDLE_DURATION_TICKS)`.

The query gains `&Position` (read) so each agent's leaf and position are visible. The `EvalContext` for predicates grows `agent_leaf: LeafAreaId`.

### `systems::movement::walk` (new)

```rust
// crates/core/src/systems/movement.rs (new)

pub const WALK_SPEED_M_PER_TICK: f32 = 80.0;       // ~5 km/h, ADR 0008's 1 sim-min/tick
pub const ARRIVE_EPSILON: f32 = 0.05;              // 5 cm

#[allow(clippy::needless_pass_by_value, reason = "bevy_ecs SystemParam: Res must be passed by value")]
pub(crate) fn walk(
    current_tick: Res<CurrentTick>,
    mut agents: Query<(&mut Position, &mut Facing, &mut CurrentAction)>,
) {
    for (mut position, mut facing, mut action) in &mut agents {
        let Some(committed) = &mut action.0 else { continue; };
        if committed.phase != Phase::Walking { continue; }
        let Some(target) = committed.target_position else {
            // Defensive: a Walking action with no target is a bug. Skip + log.
            continue;
        };

        let (next, new_facing) = step_toward(position.pos, target, WALK_SPEED_M_PER_TICK);
        position.pos = next;
        if new_facing != Vec2::ZERO {
            facing.dir = new_facing;
        }

        let dx = target.x - next.x;
        let dy = target.y - next.y;
        if dx * dx + dy * dy <= ARRIVE_EPSILON * ARRIVE_EPSILON {
            committed.phase = Phase::Performing;
            committed.started_tick = current_tick.0;
            committed.expected_end_tick = Some(
                current_tick.0 + u64::from(committed.perform_duration_ticks),
            );
        }
    }
}
```

Schedule order:

```rust
schedule.add_systems((
    systems::needs::decay,
    systems::mood::update,
    systems::decision::execute::execute,
    systems::movement::walk,           // ← new, between execute and decide
    systems::decision::decide::decide,
).chain());
```

Order rationale: `execute` retires any action that finished last tick (cleared by completion). `walk` advances any in-flight Walking actions. `decide` commits new actions for any agent whose `current_action` is now `None`. Agents who arrive at their target this tick transition to Performing inside `walk`; they don't pick a new action this tick.

### `execute` system update

`execute` checks `committed.expected_end_tick` is `Some` AND `current_tick >= it` AND `phase == Performing`. Walking actions never satisfy this — they're retired only after walking completes.

### `Predicate::Spatial(SameLeafArea)` evaluator

```rust
pub struct EvalContext<'a> {
    pub needs: &'a Needs,
    pub agent_leaf: LeafAreaId,        // ← new
    pub object_state: &'a StateMap,
    pub object_leaf: LeafAreaId,       // ← new
}

// In evaluate():
Predicate::Spatial(SpatialReq::SameLeafArea) => ctx.agent_leaf == ctx.object_leaf,
Predicate::Spatial(SpatialReq::AdjacentArea | SpatialReq::KnownPlace) => true,  // TODO
```

`decide` passes `agent_leaf = position.leaf` and, per object, `object_leaf = object.location`.

### Wire shape: `AgentSnapshot`, `ObjectSnapshot`, `Snapshot`, `WorldLayout`, `ServerMessage::Init`

```rust
// crates/core/src/snapshot.rs

pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub needs: Needs,
    pub mood: Mood,
    pub personality: Personality,
    pub leaf: LeafAreaId,                // ← new
    pub pos: Vec2,                       // ← new
    pub facing: Vec2,                    // ← new
    pub action_phase: Option<Phase>,     // ← new; None when current_action is None
    pub current_action: Option<CurrentActionView>,
}

pub struct ObjectSnapshot {              // ← new wire type
    pub id: ObjectId,
    pub type_id: ObjectTypeId,
    pub leaf: LeafAreaId,
    pub pos: Vec2,
}

pub struct Snapshot {
    pub tick: u64,
    pub agents: Vec<AgentSnapshot>,
    pub objects: Vec<ObjectSnapshot>,    // ← new; sorted by ObjectId for determinism
}

// crates/core/src/world/layout.rs (new) — lossy projection of WorldGraph for the renderer
pub struct WorldLayout {
    pub districts: Vec<District>,
    pub buildings: Vec<Building>,
    pub floors:    Vec<Floor>,
    pub leaves:    Vec<LeafArea>,
    pub default_spawn_leaf: LeafAreaId,
}

impl From<&WorldGraph> for WorldLayout { /* sorted-vec projection for determinism */ }
```

```rust
// crates/protocol/src/messages.rs

pub enum ServerMessage {
    Hello { ... },
    Init {
        current_tick: u64,
        world: WorldLayout,              // ← new
        snapshot: Snapshot,
    },
    Snapshot { snapshot: Snapshot },
}
```

`WorldLayout` is content-addressable for v0 — the same `Sim::new(seed, ...)` call always produces the same layout, so it's safe to cache client-side per session. Reconnect builds a fresh `Init` (per ADR 0013); the `world` field re-ships.

### Frontend consumers

```ts
// apps/web/src/lib/sim/reducer.ts

export type SimState = {
  status: SimStatus;
  snapshot: Snapshot | null;
  world: WorldLayout | null;            // ← new
  lastTick: number | null;
};

// in reduceServerMessage:
case "init":
  return {
    status: "connected",
    snapshot: msg.snapshot,
    world: msg.world,
    lastTick: msg.snapshot.tick,
  };
```

```tsx
// apps/web/src/components/AgentList.tsx
// New "Where" column between Doing and the personality block:

<th className="px-2 py-1 text-neutral-500" title="Leaf area : (x, y)">Where</th>
// ...
<td className="px-2 py-1 font-mono text-neutral-500">
  {agent.leaf}:({agent.pos.x.toFixed(1)}, {agent.pos.y.toFixed(1)})
</td>
```

### Determinism

- `WorldGraph::seed_v0()` is pure; allocates IDs from constants. Same seed → same layout.
- `WorldLayout::from(&WorldGraph)` sorts each vec by ID before emission. HashMap iteration order doesn't reach the wire.
- `Sim::snapshot` already sorts agents by `AgentId`; the new `objects: Vec<ObjectSnapshot>` is sorted by `ObjectId` for the same reason.
- Spawn helpers place agents at `WorldGraph::default_spawn_leaf` and a deterministic position derived from `next_agent_id` (e.g. small grid offset). No RNG draw — Personality already uses one draw per spawn; placement is structural.
- Movement is deterministic: `step_toward` is pure float math; `WALK_SPEED_M_PER_TICK` is a constant.

### Reducer test fixture growth

`reducer.test.ts`'s `fixtureSnapshot` grows `leaf: 0`, `pos: { x: 0, y: 0 }`, `facing: { x: 1, y: 0 }`, `action_phase: null`. All inline `Snapshot` literals in that file get the same additions (mirroring the personality pass's pattern). The fixture also grows `objects: []`. The reducer-test file additionally grows a new test asserting `world` lands on `Init` (a small fixture `WorldLayout` is constructed inline).

## Module changes by crate

### `gecko-sim-core`

- **New:**
  - `src/world/types.rs` — `Vec2`, `Rect2`, `OutdoorZoneKind`, `LeafKind`, `LeafArea`, `District`, `Building`, `Floor`.
  - `src/world/graph.rs` — `WorldGraph` resource + `seed_v0()`.
  - `src/world/layout.rs` — `WorldLayout` + `From<&WorldGraph>`.
  - `src/agent/spatial.rs` — `Position`, `Facing`.
  - `src/systems/movement.rs` — `walk` system + `step_toward` helper.
- **Modified:**
  - `src/world/mod.rs` — re-export the new submodules; drop `pub use glam::Vec2`.
  - `src/ids.rs` — add `id_newtype!(FloorId);`.
  - `src/lib.rs` — re-export `Position`, `Facing`, `WorldGraph`, `WorldLayout`, the new world types, and `Phase`.
  - `src/agent/mod.rs` — add `pub mod spatial; pub use spatial::{Position, Facing};`. `Gecko::current_leaf`/`position`/`facing` types stay; the schema struct is unaffected by component shape changes.
  - `src/decision/mod.rs` — `CommittedAction` grows `perform_duration_ticks` and `expected_end_tick: Option<u64>`. `Phase` gains the ts-rs derives.
  - `src/snapshot.rs` — `AgentSnapshot` grows `leaf`, `pos`, `facing`, `action_phase`. `Snapshot` grows `objects`. New `ObjectSnapshot`.
  - `src/sim.rs` — `Sim::new` inserts `WorldGraph`. `spawn_test_agent_with_needs` reads `default_spawn_leaf` and places `Position { leaf, pos: spawn_offset(id) }` + `Facing::default()`. The `snapshot` projection adds `leaf`, `pos`, `facing`, `action_phase`, and the new `objects` list. `Schedule` gains `systems::movement::walk` between `execute` and `decide`.
  - `src/systems/decision/predicates.rs` — `EvalContext` grows `agent_leaf` + `object_leaf`. `Spatial(SameLeafArea)` compares them.
  - `src/systems/decision/decide.rs` — `decide` reads `&Position`, sets `Phase::Walking` when target is non-trivial distance away, threads `agent_leaf` / `object_leaf` into the predicate context.
  - `src/systems/decision/execute.rs` — checks `expected_end_tick.is_some_and(|t| current_tick.0 >= t)` and `phase == Performing`.
  - `src/systems/mod.rs` — `pub mod movement;`.
- **Tests modified:**
  - `tests/decision.rs::agent_eats_from_fridge_when_hungry` — same outcome (hunger restores). The walking distance is zero (object spawned at agent position) so the action goes straight to Performing on tick 1; the existing tick budget covers it.
  - `tests/smoke.rs` — `let _ = Vec2::new(...)` smoke import survives the `Vec2` swap (the new local `Vec2` exposes `::new`).
  - `tests/snapshot.rs` — agents now have `leaf`/`pos`/`facing`/`action_phase`. Add assertions against `default_spawn_leaf` and the deterministic spawn offset.
  - `tests/determinism.rs` — same-seed → same-snapshot continues to hold.
  - `src/systems/decision/decide.rs::tests` and `src/systems/decision/execute.rs::tests` — agents grow a `Position` component, predicate / scoring contexts grow the new fields, expected-end-tick becomes `Some(_)`. Walking-vs-performing branch covered explicitly.
- **Tests new:**
  - `crates/core/src/world/graph.rs::tests` — `seed_v0` produces the expected counts (1 district, 1 building, 1 floor, 3 leaves), adjacency is symmetric on every edge, and `default_spawn_leaf` resolves.
  - `crates/core/src/world/layout.rs::tests` — `From<&WorldGraph>` projection sorts each vec; equal `WorldGraph`s produce equal `WorldLayout`s.
  - `crates/core/src/systems/movement.rs::tests` — agent at `(0,0)` walking to `(200,0)` (distance 200, max 80 m/tick) reaches target in 3 ticks; phase transitions Walking → Performing on the third tick; `started_tick` and `expected_end_tick` update correctly. Self-action agent never enters movement.

### `gecko-sim-protocol`

- **Modified:**
  - `src/messages.rs` — `ServerMessage::Init` grows `world: WorldLayout`. Re-exports.
  - `tests/roundtrip.rs` — fixtures grow `leaf`, `pos`, `facing`, `action_phase`, `objects`. New round-trip test for `Init { world, ... }`.

### `gecko-sim-host`

- **Modified:**
  - `src/main.rs` — uses `Sim::world_graph()` to find the spawn leaf for `spawn_one_of_each_object_type`, replacing the literal `LeafAreaId::DEFAULT`. Object positions live near the agent spawn so the existing decision-runtime test budget still completes the action chain in <15 ticks.
  - `src/ws_server.rs` — passes `WorldLayout::from(sim.world_graph())` into the `Init` message.
  - `tests/ws_smoke.rs` — assertions extend to confirm the `Init` payload carries a non-empty `world.leaves` list.

### `apps/web`

- **Modified:**
  - `src/types/sim/{Vec2,Rect2,OutdoorZoneKind,LeafKind,LeafArea,District,Building,Floor,WorldLayout,ObjectSnapshot,Phase,Position,Facing}.ts` — auto-emitted by `pnpm gen-types` (driven by `cargo test --features export-ts`).
  - `src/types/sim/{AgentSnapshot,Snapshot,ServerMessage,FloorId,DistrictId}.ts` — regenerated.
  - `src/lib/sim/reducer.ts` — `SimState` grows `world: WorldLayout | null`; `init` action stores it.
  - `src/lib/sim/reducer.test.ts` — fixture grows new `AgentSnapshot` fields and `objects: []`; new test for `init.world`.
  - `src/components/AgentList.tsx` — "Where" column.

### Determinism gates

The existing `tests/determinism.rs` test stays unchanged. With deterministic spawn offsets and `WorldGraph::seed_v0()` purity, same seed → byte-equal `Snapshot` continues to hold.

## Tests

### Rust unit tests (in-module)

#### `world::graph`

```rust
#[test]
fn seed_v0_has_three_leaves_one_district_one_building_one_floor() {
    let g = WorldGraph::seed_v0();
    assert_eq!(g.districts.len(), 1);
    assert_eq!(g.buildings.len(), 1);
    assert_eq!(g.floors.len(), 1);
    assert_eq!(g.leaves.len(), 3);   // plaza, forecourt, living-room
}

#[test]
fn seed_v0_adjacency_is_symmetric() {
    let g = WorldGraph::seed_v0();
    for (id, leaf) in &g.leaves {
        for adj in &leaf.adjacency {
            assert!(g.are_adjacent(*adj, *id), "asymmetric edge {id:?} → {adj:?}");
        }
    }
}

#[test]
fn seed_v0_default_spawn_leaf_resolves() {
    let g = WorldGraph::seed_v0();
    assert!(g.leaf(g.default_spawn_leaf).is_some());
}
```

#### `systems::movement`

```rust
#[test]
fn agent_walks_three_ticks_then_performs() {
    // distance = 200 m, speed = 80 m/tick → arrive on tick 3.
    let mut world = build_walking_world(start = (0,0), target = (200,0), perform_duration = 5);
    let mut schedule = Schedule::default();
    schedule.add_systems(walk);
    for tick in 1..=3 {
        *world.resource_mut::<CurrentTick>() = CurrentTick(tick);
        schedule.run(&mut world);
    }
    let action = world.get::<CurrentAction>(agent).unwrap().0.as_ref().unwrap();
    assert_eq!(action.phase, Phase::Performing);
    assert_eq!(action.started_tick, 3);
    assert_eq!(action.expected_end_tick, Some(3 + 5));
    let pos = world.get::<Position>(agent).unwrap();
    assert!((pos.pos.x - 200.0).abs() < ARRIVE_EPSILON);
}

#[test]
fn self_action_agent_never_enters_movement() {
    // Agent with phase=Performing & no target_position must be a no-op for `walk`.
}

#[test]
fn arrival_within_one_tick_when_close() {
    // Distance = 20m, speed = 80m/tick → arrive on first tick.
}
```

### Rust integration tests

- `tests/decision.rs` — unchanged behavior. The fridge action still completes within 15 ticks because both agent and fridge live in the spawn leaf at the spawn position (zero walking distance).
- `tests/snapshot.rs` — `assert_eq!(snap.agents[0].leaf, sim.world_graph().default_spawn_leaf)`. `assert_eq!(snap.agents[0].action_phase, None)` (no action committed yet at tick 0).
- `tests/determinism.rs` — unchanged; same seed → same snapshot.

### Protocol round-trip tests

- `agent_snapshot_with_spatial_fields_roundtrips` — `leaf`, `pos`, `facing`, `action_phase` all survive serde.
- `init_message_with_world_layout_roundtrips` — assemble a tiny `WorldLayout`, wrap in `ServerMessage::Init`, assert byte-equal after JSON round-trip.

### Frontend Vitest tests

- `init message stores world layout` — fixture `Init` carries a 2-leaf `WorldLayout`; reducer state's `world.leaves.length === 2`.
- Existing `init message preserves the personality field on the snapshot` etc. continue to pass; the new fields are added to all fixtures.

### Manual smoke

After all commits land:

1. `cargo run -p gecko-sim-host`. Logs `seed instances spawned object_count=2`.
2. `cd apps/web && pnpm dev`, open http://localhost:3000.
3. The "Where" column displays `1:(0.0, 0.0)` (or whatever the deterministic spawn offset is) for each agent on tick 0.
4. As ticks advance, agents drift toward whichever object their action picks. At 64×, the position values change visibly tick-over-tick — that's the renderer cue. Action display flips between `Eat snack (Walking)` / `Sit (Walking)` / `... (Performing)` as agents arrive and finish.
5. The frontend's React DevTools (or `console.log(state.world)`) shows the world layout structured per ADR 0007.

## Definition of done

- `cargo build --workspace` clean.
- `cargo test --workspace` clean — new unit tests in `world::graph`, `world::layout`, `systems::movement` pass; updated `decide`/`execute` tests pass; `tests/{decision,snapshot,determinism,smoke}.rs` green.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test -p gecko-sim-core --features export-ts` regenerates types; idempotent.
- `cargo test -p gecko-sim-protocol --features export-ts` regenerates types; idempotent.
- `cd apps/web && pnpm test && pnpm tsc --noEmit && pnpm lint && pnpm build` clean.
- `crates/host/tests/ws_smoke.rs` includes a `world.leaves.is_empty() == false` assertion that passes.
- Manual smoke: "Where" column shows live, deterministic positions; action display flips Walking ↔ Performing as expected.
- Multi-commit chain matching the commit-strategy section below.

## Commit strategy

Six commits:

1. `Spatial: Vec2 / Rect2 schema types + drop glam re-export`
2. `Spatial: World types + WorldGraph::seed_v0 + FloorId; unit tests`
3. `Spatial: Position / Facing components + spawn helper places at default_spawn_leaf`
4. `Spatial: Phase::Walking + movement::walk system + decide/execute integration; predicate gets agent/object leaf`
5. `Spatial: AgentSnapshot grows position fields; ObjectSnapshot + Snapshot.objects; WorldLayout + Init.world; protocol roundtrips; ts-rs regen`
6. `Spatial: frontend reducer/world state + AgentList "Where" column; reducer test fixture growth`

The natural break points are: schema (1, 2) → live ECS state (3) → behavior (4) → wire (5) → frontend (6). Each commit's `cargo test --workspace` is green; each leaves the workspace builds-clean.

`pnpm tsc --noEmit` is intentionally allowed to fail after commit 5 (frontend fixtures don't yet match the new shape) and re-greens after commit 6 — same pattern as the personality and decision-runtime passes.

## Trace to ADRs

- **ADR 0007 (world spatial model):** the v0 type tree (`District/Building/Floor/LeafArea`, `LeafKind::Room | OutdoorZone(...)`) lands here. Hierarchical pathfinding and outdoor-zone richness deferred. The 0.5m grid mentioned in 0007 is *not* enforced at v0 — agent positions are continuous f32. Snapping happens only when smart objects are placed (and v0 places them at constants).
- **ADR 0008 (time):** 1 tick = 1 sim-min; `WALK_SPEED_M_PER_TICK = 80.0` matches a 5 km/h walking pace. Phase transitions happen at tick boundaries (per ADR 0008's snapshot determinism).
- **ADR 0011 (schema):** `Position` / `Facing` ECS components are the lazy-shard projection of `Gecko::current_leaf` / `Gecko::position` / `Gecko::facing`. `CommittedAction` grows `perform_duration_ticks` per the open-question note in 0011 ("Action chaining" — the same field will become the per-step duration for chained actions).
- **ADR 0013 (transport):** renderer-facing state per-tick gets `current_leaf`, `position`, `facing`, `action_phase` (per the doc's enumerated list). Per-smart-object per-tick gets `id, location, position` (the doc also lists `visual_state`; deferred until any object actually has visual state to expose). `Init` grows `world` matching the doc's "World structure" line.

## Deferred items

| Item | Triggers landing | Lives in |
|---|---|---|
| Cross-leaf hierarchical A\* pathfinding | First scenario where agents and reachable objects span leaves | `systems::pathfinding` (new) |
| `Spatial(AdjacentArea)` and `Spatial(KnownPlace)` predicate variants | When content uses them | `predicates.rs` |
| Smart-object discovery scoping (leaf → building → district → known) | Same trigger as A* pathfinding | `systems::decision::decide` |
| `visual_state` on `ObjectSnapshot` | First object whose visible-state matters (fridge open/closed) | `snapshot.rs::ObjectSnapshot` |
| Authored world content (RON for districts/buildings/leaves) | First scenario the seed world can't represent | `crates/content`, `World::load_from_dir` |
| Three.js scene + renderer interpolation | Next pass | `apps/web/src/components/WorldScene.tsx` (new) |
| 0.5m grid snap for smart objects | When chair / table / fridge layout starts mattering visually | `WorldGraph::snap_to_grid` |
| Multi-floor stairwell / elevator connectors | Multi-floor seed buildings | `world::types::FloorConnector` |
| Save format with `Position` / `Facing` | Save/load pass (per ADR 0010) | already future-proofed by serde derives this pass |

## What this pass enables next

- **Three.js renderer pass.** All inputs are on the wire: per-tick agent + smart-object positions, world layout on connect, action phase per agent. The next pass mounts a `<canvas>` and renders.
- **Cross-leaf pathfinding pass.** Adjacency is in `WorldGraph`; the `walk` system already dispatches per-tick step. Adding hierarchical A* slots into `decide` or a precomputed-route field on `CommittedAction`.
- **Authored world content.** The pass shapes are content-loadable shapes. RON loaders mirror the pattern from `crates/content::load_from_dir`.
- **Real spatial predicates.** `SameLeafArea` is real; `AdjacentArea` is one BFS lookup away.
