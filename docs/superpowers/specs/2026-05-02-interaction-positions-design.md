# Interaction Positions Design

- **Status:** Approved
- **Date:** 2026-05-02
- **Scope:** Next spatial/movement pass after the world-scene renderer. Adds object-authored interaction positions inside a leaf area so agents walk to believable usable spots around objects instead of targeting object centers.
- **Predecessors:**
  - [`2026-04-28-spatial-pass-design.md`](2026-04-28-spatial-pass-design.md) - introduced leaf areas, `Position`, `Facing`, and `Phase::Walking`.
  - [`2026-05-02-world-scene-v0-design.md`](2026-05-02-world-scene-v0-design.md) - made current agent/object positions visible in the Three.js scene.

## Goal

Smart objects can describe one or more usable spots relative to the object. When an agent chooses an object advertisement, the decision runtime resolves the action target to the nearest currently available interaction spot, walks the agent there, and leaves the agent facing the intended direction while performing.

The visible result should be simple and immediate: a gecko sits at the usable side of a chair or stands at the fridge door instead of walking into the object's center marker.

## Non-goals

- No obstacle-aware local navigation or room walkability polygons.
- No cross-leaf route planning or door/portal traversal.
- No long-lived reservation system independent of current actions.
- No frontend protocol churn unless an existing snapshot field naturally reflects the new position/facing.
- No authored meshes, animations, or pose system. Sitting/standing is represented by position, facing, and action display text for now.
- No multi-agent negotiation beyond filtering occupied spots during decision.

## Recommended Approach

Add an `InteractionSpot` schema to the smart-object contract and resolve object actions to interaction spots during decision.

Object types define default spots that all instances inherit. Each spot has:

- a stable local spot ID
- an offset from the object position in leaf-local meters
- a facing direction
- an optional label/kind for debugging and future renderer affordances
- an optional radius/tolerance if needed by movement tests

For v0, occupancy is derived from current actions rather than stored separately. A spot is unavailable if another agent is currently walking to or performing at the same object/spot. If all spots for an object are occupied, that advertisement is filtered out for this decision pass. If an object type defines no spots, the runtime keeps today's object-center fallback so existing content remains valid.

## Architecture

### Schema

Add a new type near the smart-object schema:

```rust
pub struct InteractionSpot {
    pub id: InteractionSpotId,
    pub offset: Vec2,
    pub facing: Vec2,
    pub label: Option<String>,
}
```

`InteractionSpotId` should be a small new ID type in `ids.rs`, exported to TypeScript only if it naturally appears on wire-facing types. The initial design does not require it on snapshots.

`ObjectType` grows:

```rust
pub interaction_spots: Vec<InteractionSpot>,
```

Content validation should reject non-finite offsets, zero or non-finite facing vectors, duplicate spot IDs within an object type, and spots that would land outside the leaf bbox for any seeded instance when that can be checked.

### Runtime Target Resolution

Introduce a focused resolver in the decision system, separate from scoring:

```rust
resolve_interaction_target(
    object: &SmartObject,
    object_type: &ObjectType,
    agent_position: &Position,
    occupied_spots: &OccupiedInteractionSpots,
) -> InteractionTarget
```

The resolver:

1. Converts each spot's local offset into a leaf-local world position by adding it to `SmartObject::position`.
2. Normalizes the spot facing vector.
3. Filters out occupied spots for that `(object_id, spot_id)`.
4. Picks the remaining spot closest to the agent's current position.
5. Falls back to the object center with facing toward the object if the type has no spots.
6. Returns `None` when spots exist but all are occupied, causing the advertisement to be filtered out.

### Committed Action Shape

`CommittedAction` should remember the resolved spot and arrival facing:

```rust
pub target_spot: Option<InteractionSpotId>,
pub target_facing: Option<Vec2>,
```

`target_position` remains the movement destination. This keeps the existing movement system contract intact while giving occupancy checks and arrival-facing logic enough information.

### Movement

`systems::movement::walk` still moves toward `target_position`. On arrival it transitions to `Phase::Performing` as today, then applies `target_facing` if present. During walking, facing continues to follow movement direction; after arrival, facing becomes the interaction-facing direction.

### Occupancy

Occupancy is computed from active `CurrentAction` components:

- object-targeted actions with `phase == Walking` or `phase == Performing`
- a `target_spot` value
- the same `ObjectId`

This keeps the first pass deterministic and avoids persistent reservation cleanup. If an agent completes or abandons the action, the spot naturally becomes available because the current action disappears.

## Data Flow

1. `decide` builds object advertisement candidates as today.
2. Predicate and score checks still run against the object/ad pair.
3. For each surviving candidate, target resolution tries to assign an available interaction spot.
4. Candidates with no available spot are discarded.
5. Weighted pick chooses among candidates.
6. The committed object action stores `target_position`, `target_spot`, and `target_facing`.
7. `movement::walk` moves to `target_position`, then sets facing to `target_facing` on arrival.
8. `execute` applies effects normally after the perform duration elapses.
9. `snapshot` reflects the improved position and facing through existing agent fields.

## Seed Content

Add default spots to the existing seed objects:

- **Chair:** one front-facing sit spot offset from the chair center. Single occupancy creates a visible "chair is in use" behavior.
- **Fridge:** one door spot offset from the front of the fridge, facing toward the object.

Exact offsets can be tuned in tests, but should land on the current 0.5m object-placement grid where practical.

## Error Handling

- Object types with no interaction spots use the object-center fallback.
- Object types with invalid interaction spots fail content validation.
- If a resolved spot lands outside the object's leaf bbox at runtime, log a warning and ignore that spot.
- If all spots are occupied, filter the advertisement for that decision pass.
- If a committed walking action has a missing `target_position`, keep today's warning/no-op behavior.
- If `target_facing` is absent, movement keeps its current arrival-facing behavior.

## Testing

Core unit tests:

- `InteractionSpot` IDs are unique within an object type during validation.
- invalid spots fail validation: non-finite offset, non-finite facing, zero facing.
- resolver converts object position plus offset into the expected target.
- resolver normalizes facing.
- resolver picks the nearest available spot.
- resolver filters occupied spots.
- resolver returns object-center fallback when the object type has no spots.
- resolver returns `None` when spots exist but all are occupied.

Decision/movement tests:

- a hungry agent targets the fridge interaction spot, not the fridge center.
- an arriving agent faces the fridge while performing.
- two agents do not choose the same single-occupancy chair spot while one is walking to or using it.
- existing no-spot test objects still behave with the object-center fallback.

Integration tests:

- seed fridge "Eat snack" still restores hunger and generates its routine memory.
- snapshots expose improved agent position/facing without adding new wire fields.
- determinism remains stable for two sims with the same seed and tick sequence.

Suggested verification:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd apps/web && pnpm test
```

The websocket smoke test may still need permission to bind localhost in this sandbox.

## Follow-up

This pass deliberately stops at room-local usable positions. The natural follow-ups are:

- local navigation around room obstacles
- cross-leaf routes through doors/portals
- renderer affordances for selected interaction spots
- pose/state display for sitting, eating, and similar action-specific presentations
