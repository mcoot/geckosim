# Interaction Positions Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add object-authored interaction spots so agents walk to believable usable positions around smart objects and face the intended direction while performing.

**Architecture:** Extend the smart-object schema with default interaction spots, validate them in content loading, and resolve object actions to available spots during decision. Keep movement's existing `target_position` contract, adding only optional committed-action metadata for `target_spot` and arrival-facing.

**Tech Stack:** Rust 2024 workspace, Bevy ECS, RON content, serde, ts-rs generated TypeScript IDs, cargo tests/clippy, jj version control.

**Reference:** Spec at [`docs/superpowers/specs/2026-05-02-interaction-positions-design.md`](../specs/2026-05-02-interaction-positions-design.md). Relevant predecessors: [`2026-04-28-spatial-pass-design.md`](../specs/2026-04-28-spatial-pass-design.md), [`2026-05-02-world-scene-v0-design.md`](../specs/2026-05-02-world-scene-v0-design.md).

**VCS note:** This repo uses `jj` (jujutsu), colocated with git. **Never use raw `git`.** Each task below starts a new jj change with `jj new -m "<message>"`. There is no staging area and no `git commit`.

---

## File Structure

**Create:**
- `crates/core/src/systems/decision/interaction.rs` - pure-ish target resolution helpers: spot world position, facing normalization, occupancy collection, nearest available spot selection.

**Modify:**
- `crates/core/src/ids.rs` - add `InteractionSpotId`.
- `crates/core/src/object/mod.rs` - add `InteractionSpot` and `ObjectType::interaction_spots` with serde default.
- `crates/content/src/error.rs` - add content-validation error variants for invalid/duplicate spots.
- `crates/content/src/validate.rs` - validate duplicate spot IDs and invalid spot vectors.
- `crates/content/tests/validation.rs` - add validation tests and update object-type fixtures if needed.
- `crates/core/src/decision/mod.rs` - add `target_spot` and `target_facing` to `CommittedAction`.
- `crates/core/src/systems/decision/mod.rs` - expose the new `interaction` module.
- `crates/core/src/systems/decision/decide.rs` - compute occupied spots, resolve targets, filter fully occupied object actions, and commit spot/facing metadata.
- `crates/core/src/systems/movement.rs` - apply `target_facing` when a walking action arrives.
- `crates/core/src/systems/decision/execute.rs` - update test `CommittedAction` literals.
- `crates/core/tests/decision.rs` - add end-to-end fridge/chair behavior tests.
- `crates/core/tests/snapshot.rs` - add or adjust assertions for improved facing if needed.
- `content/object_types/chair.ron` - add one sit spot.
- `content/object_types/fridge.ron` - add one door spot.
- `apps/web/src/types/sim/InteractionSpotId.ts` - generated if `InteractionSpotId` uses the existing exported ID macro.
- Any generated type barrel/readme updates produced by `cd apps/web && pnpm gen-types`.

**Do not modify:**
- `apps/web/src/lib/world-scene/*` - existing renderer should observe improved positions/facing through current snapshot fields.
- Protocol messages - no new websocket shape is expected.

---

## Chunk 1: Schema and Content Validation

### Task 1: Add Interaction Spot Schema

**Files:**
- Modify: `crates/core/src/ids.rs`
- Modify: `crates/core/src/object/mod.rs`
- Test: `crates/content/tests/validation.rs`

- [ ] **Step 1.1: Start the task change**

```bash
jj new -m "Interaction positions: add spot schema"
```

- [ ] **Step 1.2: Write a failing parse/fixture test**

In `crates/content/tests/validation.rs`, add a small valid object type that includes `interaction_spots` and assert `load_from_dir` succeeds and preserves the spot:

```rust
#[test]
fn object_type_with_interaction_spot_loads() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
ObjectType(
    id: ObjectTypeId(9),
    display_name: "Bench",
    mesh_id: MeshId(9),
    default_state: {},
    interaction_spots: [
        InteractionSpot(
            id: InteractionSpotId(1),
            offset: Vec2(x: 0.0, y: -1.0),
            facing: Vec2(x: 0.0, y: 1.0),
            label: Some("front"),
        ),
    ],
    advertisements: [],
)
"#;
    write_object_type(tmp.path(), "bench.ron", body);
    let bundle = load_from_dir(tmp.path()).expect("valid spot object type loads");
    let object_type = bundle.object_types.get(&gecko_sim_core::ids::ObjectTypeId::new(9)).unwrap();
    assert_eq!(object_type.interaction_spots.len(), 1);
}
```

- [ ] **Step 1.3: Run the focused test and verify RED**

Run:

```bash
cargo test -p gecko-sim-content object_type_with_interaction_spot_loads
```

Expected: FAIL because `InteractionSpot`, `InteractionSpotId`, and/or `ObjectType::interaction_spots` do not exist.

- [ ] **Step 1.4: Add the ID and schema**

In `crates/core/src/ids.rs`, add:

```rust
id_newtype!(InteractionSpotId);
```

In `crates/core/src/object/mod.rs`, import the ID and define:

```rust
use crate::ids::{AdvertisementId, InteractionSpotId, LeafAreaId, ObjectId, ObjectTypeId, OwnerRef};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionSpot {
    pub id: InteractionSpotId,
    pub offset: Vec2,
    pub facing: Vec2,
    pub label: Option<String>,
}
```

Then grow `ObjectType`:

```rust
#[serde(default)]
pub interaction_spots: Vec<InteractionSpot>,
```

Keep `#[serde(default)]` so existing authored content and tests without spots still parse and use the object-center fallback.

- [ ] **Step 1.5: Update compile-time object literals**

Search:

```bash
rg "ObjectType \\{" crates
```

Add `interaction_spots: vec![],` to Rust `ObjectType` literals in tests/helpers that do not use the RON loader.

- [ ] **Step 1.6: Run the focused test and verify GREEN**

Run:

```bash
cargo test -p gecko-sim-content object_type_with_interaction_spot_loads
```

Expected: PASS.

- [ ] **Step 1.7: Run core compile tests**

Run:

```bash
cargo test -p gecko-sim-core --lib
```

Expected: PASS after all object-type literals are updated.

### Task 2: Validate Spot IDs and Vectors

**Files:**
- Modify: `crates/content/src/error.rs`
- Modify: `crates/content/src/validate.rs`
- Modify: `crates/content/tests/validation.rs`

- [ ] **Step 2.1: Start the task change**

```bash
jj new -m "Interaction positions: validate authored spots"
```

- [ ] **Step 2.2: Write failing validation tests**

Add tests for duplicate spot IDs, non-finite offset/facing, and zero facing:

```rust
#[test]
fn duplicate_interaction_spot_id_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"
ObjectType(
    id: ObjectTypeId(1),
    display_name: "Chair",
    mesh_id: MeshId(1),
    default_state: {},
    interaction_spots: [
        InteractionSpot(id: InteractionSpotId(1), offset: Vec2(x: 0.0, y: -1.0), facing: Vec2(x: 0.0, y: 1.0), label: None),
        InteractionSpot(id: InteractionSpotId(1), offset: Vec2(x: 1.0, y: 0.0), facing: Vec2(x: -1.0, y: 0.0), label: None),
    ],
    advertisements: [],
)
"#;
    write_object_type(tmp.path(), "chair.ron", body);
    let err = load_from_dir(tmp.path()).expect_err("should fail");
    assert!(matches!(err, ContentError::DuplicateInteractionSpotId { .. }), "got {err:?}");
}
```

Add similar tests that expect `ContentError::InvalidInteractionSpotVector { .. }` for:

```ron
offset: Vec2(x: inf, y: 0.0)
facing: Vec2(x: 0.0, y: 0.0)
facing: Vec2(x: NaN, y: 1.0)
```

If RON parsing rejects `inf`/`NaN` before validation, construct the invalid `ObjectType` directly in a unit test inside `validate.rs` instead.

- [ ] **Step 2.3: Run focused tests and verify RED**

Run:

```bash
cargo test -p gecko-sim-content interaction_spot
```

Expected: FAIL because the new error variants and validation logic do not exist.

- [ ] **Step 2.4: Add error variants**

In `crates/content/src/error.rs`, import `InteractionSpotId` and add:

```rust
#[error("duplicate InteractionSpotId {spot:?} within ObjectType {object_type:?} in {path}", path = path.display())]
DuplicateInteractionSpotId {
    object_type: ObjectTypeId,
    spot: InteractionSpotId,
    path: PathBuf,
},

#[error("invalid interaction spot {spot:?} on ObjectType {object_type:?} ({path}): {reason}", path = path.display())]
InvalidInteractionSpotVector {
    object_type: ObjectTypeId,
    spot: InteractionSpotId,
    reason: &'static str,
    path: PathBuf,
},
```

- [ ] **Step 2.5: Implement validation**

In `validate_object_types`, before validating ads for each object type, call a new helper:

```rust
fn validate_interaction_spots(path: &Path, ot: &ObjectType) -> Result<(), ContentError> {
    let mut seen = HashSet::new();
    for spot in &ot.interaction_spots {
        if !seen.insert(spot.id) {
            return Err(ContentError::DuplicateInteractionSpotId {
                object_type: ot.id,
                spot: spot.id,
                path: path.to_path_buf(),
            });
        }
        if !spot.offset.x.is_finite() || !spot.offset.y.is_finite() {
            return Err(ContentError::InvalidInteractionSpotVector {
                object_type: ot.id,
                spot: spot.id,
                reason: "offset must be finite",
                path: path.to_path_buf(),
            });
        }
        if !spot.facing.x.is_finite() || !spot.facing.y.is_finite() {
            return Err(ContentError::InvalidInteractionSpotVector {
                object_type: ot.id,
                spot: spot.id,
                reason: "facing must be finite",
                path: path.to_path_buf(),
            });
        }
        if spot.facing.x == 0.0 && spot.facing.y == 0.0 {
            return Err(ContentError::InvalidInteractionSpotVector {
                object_type: ot.id,
                spot: spot.id,
                reason: "facing must be non-zero",
                path: path.to_path_buf(),
            });
        }
    }
    Ok(())
}
```

- [ ] **Step 2.6: Run validation tests**

Run:

```bash
cargo test -p gecko-sim-content validation
```

Expected: PASS.

---

## Chunk 2: Target Resolution and Movement

### Task 3: Add Pure Interaction Target Resolver

**Files:**
- Create: `crates/core/src/systems/decision/interaction.rs`
- Modify: `crates/core/src/systems/decision/mod.rs`

- [ ] **Step 3.1: Start the task change**

```bash
jj new -m "Interaction positions: resolve available targets"
```

- [ ] **Step 3.2: Write failing resolver tests**

Create `crates/core/src/systems/decision/interaction.rs` with tests first. Cover:

- object position plus local offset becomes target position
- facing is normalized
- nearest available spot wins
- occupied spots are ignored
- no spots falls back to object center with no target spot/facing
- all spots occupied returns `None`

Use a compact expected API:

```rust
#[test]
fn resolver_picks_nearest_available_spot() {
    let object = smart_object_at(Vec2::new(10.0, 10.0));
    let object_type = object_type_with_spots(vec![
        spot(InteractionSpotId::new(1), Vec2::new(0.0, -1.0), Vec2::new(0.0, 2.0)),
        spot(InteractionSpotId::new(2), Vec2::new(4.0, 0.0), Vec2::new(-2.0, 0.0)),
    ]);
    let agent = Position { leaf: object.location, pos: Vec2::new(11.0, 8.0) };
    let occupied = OccupiedInteractionSpots::default();

    let resolved = resolve_interaction_target(&object, &object_type, &agent, &occupied, None)
        .expect("one spot available");

    assert_eq!(resolved.spot, Some(InteractionSpotId::new(1)));
    assert_eq!(resolved.position, Vec2::new(10.0, 9.0));
    assert_eq!(resolved.facing, Some(Vec2::new(0.0, 1.0)));
}
```

- [ ] **Step 3.3: Run resolver tests and verify RED**

Run:

```bash
cargo test -p gecko-sim-core systems::decision::interaction
```

Expected: FAIL until the module and API exist.

- [ ] **Step 3.4: Implement resolver types**

Recommended shapes:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedInteractionTarget {
    pub position: Vec2,
    pub spot: Option<InteractionSpotId>,
    pub facing: Option<Vec2>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct OccupiedInteractionSpots {
    occupied: HashSet<(ObjectId, InteractionSpotId)>,
}
```

Add methods:

```rust
impl OccupiedInteractionSpots {
    pub(crate) fn insert(&mut self, object: ObjectId, spot: InteractionSpotId) { ... }
    pub(crate) fn contains(&self, object: ObjectId, spot: InteractionSpotId) -> bool { ... }
}
```

- [ ] **Step 3.5: Implement resolver**

Signature:

```rust
pub(crate) fn resolve_interaction_target(
    object: &SmartObject,
    object_type: &ObjectType,
    agent_position: &Position,
    occupied: &OccupiedInteractionSpots,
    leaf_bbox: Option<Rect2>,
) -> Option<ResolvedInteractionTarget>
```

Rules:

- if `object_type.interaction_spots.is_empty()`, return object center fallback
- compute `position = object.position + spot.offset`
- skip occupied spots
- skip spots outside `leaf_bbox` when provided; log with `tracing::warn!`
- normalize facing with a small helper; content validation should have rejected zero vectors, but keep runtime defensive
- choose the smallest squared distance to `agent_position.pos`; tie-break by `spot.id`

- [ ] **Step 3.6: Run resolver tests**

Run:

```bash
cargo test -p gecko-sim-core systems::decision::interaction
```

Expected: PASS.

### Task 4: Wire Resolver into Decisions

**Files:**
- Modify: `crates/core/src/decision/mod.rs`
- Modify: `crates/core/src/systems/decision/decide.rs`
- Modify: `crates/core/src/systems/decision/execute.rs`
- Modify: `crates/core/src/systems/movement.rs` test literals as needed

- [ ] **Step 4.1: Start the task change**

```bash
jj new -m "Interaction positions: commit resolved action targets"
```

- [ ] **Step 4.2: Add failing decision tests**

In `decide.rs` tests, add:

- object with one spot commits `target_position` to spot, not object center
- second agent does not pick an already occupied spot
- object with no spots still commits object center fallback

Use direct ECS tests before integration tests. Seed an occupied spot by spawning another agent with:

```rust
CurrentAction(Some(CommittedAction {
    action: ActionRef::Object { object: ObjectId::new(0), ad: AdvertisementId::new(1) },
    started_tick: 0,
    expected_end_tick: None,
    phase: Phase::Walking,
    target_position: Some(Vec2::new(0.0, -1.0)),
    target_spot: Some(InteractionSpotId::new(1)),
    target_facing: Some(Vec2::new(0.0, 1.0)),
    perform_duration_ticks: 10,
}))
```

- [ ] **Step 4.3: Run decision tests and verify RED**

Run:

```bash
cargo test -p gecko-sim-core systems::decision::decide
```

Expected: FAIL because `CommittedAction` lacks the new fields and decide does not call the resolver.

- [ ] **Step 4.4: Grow `CommittedAction`**

In `crates/core/src/decision/mod.rs`, import `InteractionSpotId` and add:

```rust
pub target_spot: Option<InteractionSpotId>,
pub target_facing: Option<Vec2>,
```

Update all `CommittedAction` literals from `rg "CommittedAction \\{" crates` with `target_spot: None` and `target_facing: None` unless the test is specifically asserting spot behavior.

- [ ] **Step 4.5: Compute occupied spots in `decide`**

Change the system to use `ParamSet` so one query can read all `CurrentAction` components before the mutable deciding query runs:

```rust
use bevy_ecs::system::{ParamSet, Query, Res, ResMut};
```

Recommended helper:

```rust
fn collect_occupied_spots(actions: Query<&CurrentAction>) -> OccupiedInteractionSpots {
    let mut occupied = OccupiedInteractionSpots::default();
    for current in &actions {
        let Some(action) = &current.0 else { continue; };
        let ActionRef::Object { object, .. } = action.action else { continue; };
        if !matches!(action.phase, Phase::Walking | Phase::Performing) { continue; }
        if let Some(spot) = action.target_spot {
            occupied.insert(object, spot);
        }
    }
    occupied
}
```

- [ ] **Step 4.6: Resolve targets while building candidates**

Change scored candidates to carry the resolved target:

```rust
let mut scored: Vec<(ObjectId, ObjectTypeId, AdvertisementId, u32, ResolvedInteractionTarget, f32)> = Vec::new();
```

For each predicate-passing ad:

```rust
let leaf_bbox = world.leaf(object.location).map(|leaf| leaf.bbox);
let Some(target) = resolve_interaction_target(object, object_type, position, &occupied, leaf_bbox) else {
    continue;
};
```

When committing:

```rust
target_position: Some(target.position),
target_spot: target.spot,
target_facing: target.facing,
```

Use `target.position` for the existing distance/already-there check.

- [ ] **Step 4.7: Run decision tests**

Run:

```bash
cargo test -p gecko-sim-core systems::decision::decide
```

Expected: PASS.

### Task 5: Apply Arrival Facing in Movement

**Files:**
- Modify: `crates/core/src/systems/movement.rs`

- [ ] **Step 5.1: Start the task change**

```bash
jj new -m "Interaction positions: face target on arrival"
```

- [ ] **Step 5.2: Write failing movement test**

Add a test to `movement.rs`:

```rust
#[test]
fn arrival_applies_target_facing() {
    let (mut world, agent) = build(Vec2::ZERO, Vec2::new(10.0, 0.0), 4);
    world.get_mut::<CurrentAction>(agent).unwrap().0.as_mut().unwrap().target_facing =
        Some(Vec2::new(0.0, 1.0));
    let mut sched = Schedule::default();
    sched.add_systems(walk);
    *world.resource_mut::<CurrentTick>() = CurrentTick(1);
    sched.run(&mut world);
    assert_eq!(world.get::<Facing>(agent).unwrap().dir, Vec2::new(0.0, 1.0));
}
```

- [ ] **Step 5.3: Run movement tests and verify RED**

Run:

```bash
cargo test -p gecko-sim-core systems::movement
```

Expected: FAIL until `walk` applies `target_facing`.

- [ ] **Step 5.4: Apply target facing on arrival**

In the arrival branch after setting phase/end tick:

```rust
if let Some(target_facing) = committed.target_facing {
    facing.dir = target_facing;
}
```

Because content/runtime normalization already happened, do not renormalize here.

- [ ] **Step 5.5: Run movement tests**

Run:

```bash
cargo test -p gecko-sim-core systems::movement
```

Expected: PASS.

---

## Chunk 3: Seed Content and Integration

### Task 6: Add Seed Fridge and Chair Spots

**Files:**
- Modify: `content/object_types/fridge.ron`
- Modify: `content/object_types/chair.ron`
- Test: `crates/content/tests/seed_loads.rs`

- [ ] **Step 6.1: Start the task change**

```bash
jj new -m "Interaction positions: add seed object spots"
```

- [ ] **Step 6.2: Add spots to content**

In `content/object_types/fridge.ron`, add after `default_state`:

```ron
interaction_spots: [
    InteractionSpot(
        id: InteractionSpotId(1),
        offset: Vec2(x: 0.0, y: -1.0),
        facing: Vec2(x: 0.0, y: 1.0),
        label: Some("door"),
    ),
],
```

In `content/object_types/chair.ron`, add:

```ron
interaction_spots: [
    InteractionSpot(
        id: InteractionSpotId(1),
        offset: Vec2(x: 0.0, y: -0.75),
        facing: Vec2(x: 0.0, y: 1.0),
        label: Some("seat"),
    ),
],
```

- [ ] **Step 6.3: Run seed content load tests**

Run:

```bash
cargo test -p gecko-sim-content seed
```

Expected: PASS.

### Task 7: Add End-to-End Behavior Tests

**Files:**
- Modify: `crates/core/tests/decision.rs`
- Modify: `crates/core/tests/snapshot.rs` if useful

- [ ] **Step 7.1: Start the task change**

```bash
jj new -m "Interaction positions: cover end-to-end behavior"
```

- [ ] **Step 7.2: Extend hungry fridge test**

In `crates/core/tests/decision.rs`, after ticks complete, assert the agent stood at the fridge spot and faced the fridge:

```rust
assert_eq!(agent.pos, Vec2::new(0.0, -1.0));
assert_eq!(agent.facing, Vec2::new(0.0, 1.0));
```

If needs decay makes exact completion timing awkward, assert immediately after arrival in a focused sim setup or use an epsilon helper.

- [ ] **Step 7.3: Add single-occupancy chair scenario**

Create a test with two low-comfort agents and one chair spot. The first agent should commit the chair action; the second should not commit to the same object/spot while the first is `Walking` or `Performing`. Prefer asserting `target_spot` through a core helper only if needed; otherwise assert the second agent idles or chooses another available object.

- [ ] **Step 7.4: Run integration tests**

Run:

```bash
cargo test -p gecko-sim-core --test decision
cargo test -p gecko-sim-core --test snapshot
```

Expected: PASS.

### Task 8: Regenerate Types and Run Full Verification

**Files:**
- Possible generated: `apps/web/src/types/sim/InteractionSpotId.ts`
- Possible generated updates in `apps/web/src/types/sim/*.ts`

- [ ] **Step 8.1: Start the task change**

```bash
jj new -m "Interaction positions: regenerate generated types"
```

- [ ] **Step 8.2: Run type generation**

Run:

```bash
cd apps/web
pnpm gen-types
```

Expected: generated TypeScript remains idempotent except for `InteractionSpotId.ts` if the new ID exports.

- [ ] **Step 8.3: Run Rust verification**

Run:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS. If the websocket smoke test fails because localhost binding is blocked by sandbox policy, rerun the relevant command with escalation.

- [ ] **Step 8.4: Run frontend verification**

Run:

```bash
cd apps/web
pnpm test
pnpm lint
pnpm build
```

Expected: PASS. No renderer code should need to change; it should observe the improved snapshot positions and facing.

- [ ] **Step 8.5: Review jj status**

Run:

```bash
jj st
```

Expected: only intentional schema, content, test, generated-type, and doc files changed across the jj changes.

---

## Execution Notes

- Use TDD: every behavior step starts with a failing test.
- Keep resolver logic in `interaction.rs`; keep `decide.rs` focused on candidate construction and commitment.
- Do not introduce persistent reservation state in this pass.
- Do not add protocol fields unless implementation proves existing snapshot position/facing cannot carry the feature.
- For floating point assertions, use exact values only when constructed from literals without arithmetic; otherwise use epsilon checks.
