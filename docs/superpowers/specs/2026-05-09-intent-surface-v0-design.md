# Intent surface v0 - hybrid movement intent

- **Date:** 2026-05-09
- **Status:** Draft
- **Scope:** Eleventh implementation pass. Makes moving geckos visibly intentional in the existing Three.js world scene.
- **Predecessor:** [`2026-05-02-world-scene-v0-design.md`](2026-05-02-world-scene-v0-design.md) and [`2026-05-02-interaction-positions-design.md`](2026-05-02-interaction-positions-design.md).

## Goal

Make current gecko movement understandable without expanding the simulation navigation model.

End state:

1. Walking geckos show faint in-scene intent hints from their current position toward their committed target position.
2. Clicking a gecko selects it and reveals full intent detail: name, current action, phase, action progress, target label, and a stronger route/target highlight.
3. Performing geckos keep action/progress visible when selected. They may retain target context, but they do not need a route line if they are already at the target.
4. Idle or actionless geckos show no route. When selected, the detail UI makes the idle/waiting state clear.
5. The frontend continues to consume `Snapshot` and `WorldLayout`; no request/response inspection flow is introduced.
6. Smooth interpolation, cross-leaf pathfinding, waypoints, and decision-reason telemetry remain follow-ups.

Success looks like this: when Alice starts walking to the fridge, the viewer can see where she is going, what she intends to do there, and how far through the action she is once she arrives.

## Non-goals

- **No cross-leaf route planning.** This pass draws the committed direct target already produced by the decision system.
- **No smooth interpolation.** Agents may still update at snapshot cadence; this pass is about legibility of intent, not motion polish.
- **No decision explanation.** "Why did this action win?" needs explicit scoring telemetry and is a later inspection pass.
- **No new websocket message type.** Intent data rides on the existing snapshot stream.
- **No authored meshes, icons, or animation rigs.** Routes, target markers, and panels use simple generated geometry and UI.

## Architecture

### Boundary 1: Committed action to snapshot action view

Extend `CurrentActionView` in `crates/core/src/snapshot.rs` so object-targeted committed actions expose the minimal intent data the renderer needs:

- `display_name`: existing human action label.
- `fraction_complete`: existing action progress.
- `phase`: current `Phase`.
- `target_object_id`: object id for object actions, absent for self actions.
- `target_position`: committed interaction position, absent when none exists.
- `target_label`: best available human label for the target. For v0 this can be conservative, such as the object type display name when available.

`AgentSnapshot.action_phase` may remain for compatibility, but `CurrentActionView.phase` lets selected UI consume one coherent action object.

`project_current_action` is the only projection point that needs to understand `CommittedAction`. It already looks up object type and advertisement display names, so it can also include object id, target position, and target label without introducing a new query path.

### Boundary 2: Wire types to render model

Regenerate TypeScript types after the Rust snapshot shape changes.

In `apps/web/src/lib/world-scene/model.ts`, extend `AgentRenderModel` with optional intent data:

- target ground point
- target object id
- target label
- action label
- phase
- progress fraction

The projection layer owns coordinate conversion from sim `Vec2` to Three ground coordinates, just as it does for agent/object positions today. Missing action or missing target data should produce `intent: null`, not a crash.

### Boundary 3: Render model to Three scene graph

`apps/web/src/lib/world-scene/scene.ts` adds simple route and target objects:

- Faint route lines for all walking agents with target positions.
- Stronger selected route line for the selected agent.
- Target marker or halo for the selected agent's target position or target object.
- Stable names such as `intent-route:<agentId>` and `intent-target:<agentId>`.
- Stable `userData` with `agentId`, `targetObjectId`, and `kind` for future picking/debugging.

The existing root repopulation strategy can remain. It already clears stale children before rebuilding; tests should cover stale selected artifacts too.

### Boundary 4: Runtime picking and React selection

`WorldSceneRuntime` gets a selection-aware update API and click picking callback:

- Runtime accepts `selectedAgentId` when updating the scene.
- Runtime exposes `onSelectAgent(agentId)` for pointer hits on agent meshes.
- `WorldScene.tsx` owns selected agent id in React state and derives selected detail from the latest snapshot.
- If the selected agent disappears, React clears or ignores the stale selection.

The selected detail UI should be compact and operational rather than decorative: name, action/phase, progress bar, and target label. It should not duplicate the full debug table.

## Error Handling

- Missing target position: do not draw route lines; selected detail still shows the action label and phase.
- Missing target label: fall back to `target_object_id` or "target" in UI.
- Runtime/WebGL unavailable: existing fallback remains valid. The selected detail should not assume a runtime exists.
- Click misses: no selection change unless the user clicks a selectable gecko mesh.
- Stale selection: if the selected agent id is not present in the latest snapshot, hide the detail panel.

## Testing Strategy

Use focused tests at each boundary:

1. Rust snapshot tests assert object actions include phase, target object id, target position, target label, and progress; self actions leave target fields empty.
2. Protocol/roundtrip or snapshot serde tests continue to cover the expanded wire shape.
3. TypeScript model tests assert intent projection, coordinate conversion, and null behavior for missing action/target data.
4. Three scene tests assert faint walking routes, stronger selected routes, selected target markers, stable names/userData, and stale artifact cleanup on update.
5. React tests mock the runtime and cover no selection, selected walking gecko, selected performing gecko, and stale selection.
6. Manual or browser smoke verification confirms the canvas renders, geckos remain pickable, and selecting a moving gecko shows action/progress detail.

Full verification target for the implementation pass:

```bash
cd apps/web
pnpm test
pnpm lint
pnpm build
cd ../..
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`cargo test --workspace` may require permission to bind localhost for the host WebSocket smoke test in the current sandbox.

## Follow-up

Natural next passes:

- Smooth interpolation between snapshots.
- Cross-leaf route planning and route visualization.
- Decision-reason telemetry for "why this action won".
- Richer object labels and object-specific visual affordances.
