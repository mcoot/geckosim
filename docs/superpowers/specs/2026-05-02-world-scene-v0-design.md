# World scene v0 - Three.js observable sim view

- **Date:** 2026-05-02
- **Status:** Draft
- **Scope:** Tenth implementation pass. Adds the first real renderer surface to the Next.js frontend using the spatial data already exposed by the previous pass.
- **Predecessor:** [`2026-04-28-spatial-pass-design.md`](2026-04-28-spatial-pass-design.md) - `Init.world`, `Snapshot.objects`, and per-agent `leaf`, `pos`, `facing`, and `action_phase` now exist on the wire.

## Goal

End state:

1. `apps/web` depends on Three.js.
2. The frontend has a real Three.js scene, not a faux 2D canvas. The default user view is top-down and quiet, but the internals use a normal 3D scene, camera, meshes, and controls so later full-camera work extends the same surface.
3. `WorldScene` consumes existing `SimState.world` and `SimState.snapshot` only. No Rust, protocol, or generated type changes are needed.
4. Leaf areas render as flat floor/zone planes on the X/Z ground plane. Sim `Vec2 { x, y }` maps to Three `{ x, z }`; Three `y` is reserved for height.
5. Smart objects render as simple stable markers. Agents render as directional markers with distinct styling for idle, `Walking`, and `Performing`.
6. Rendered meshes carry stable `userData` IDs (`agentId`, `objectId`, `leafId`) so the next inspection pass can add picking without rewriting the scene graph.
7. The dashboard makes the scene primary: header/status, controls, world scene, then the existing `AgentList` as supporting debug data.
8. Empty, disconnected, or not-yet-initialized states show a stable placeholder instead of a blank or crashing canvas.

## Non-goals

- **No Rust wire changes.** The current `WorldLayout` and `Snapshot` are enough for v0.
- **No server-backed inspection UI.** This pass prepares mesh IDs and metadata for picking, but the "click a gecko, explain it" panel and any request/response messages are the next pass.
- **No full free-camera product UI.** The scene uses a real 3D camera and controls, but the default camera remains a top-down observation view.
- **No interpolation between snapshots.** Agents jump to the latest sampled position for v0. Smooth interpolation belongs in a later rendering polish pass.
- **No authored meshes, textures, animation rigs, or object catalog display mapping.** v0 uses simple geometry and current wire fields (`type_id`, positions, action phase).
- **No area-of-interest culling or performance tuning beyond sane disposal and resize handling.** Current seed scale is tiny; larger-world optimization can follow actual pressure.

## Architecture

### Boundary 1: Wire data to render model

Create a pure projection layer under `apps/web/src/lib/world-scene/`. It accepts generated TypeScript wire types and returns plain render data:

- leaf planes: `id`, display name, center, size, kind, color
- objects: `id`, `typeId`, `leaf`, position, color
- agents: `id`, name, `leaf`, position, heading angle, phase, color

This layer owns coordinate conversion from sim 2D to Three 3D:

```ts
// Sim meters: { x, y }
// Three world: x is east/west, y is height, z is north/south.
{ x: sim.x, y: 0, z: sim.y }
```

Tests assert exact output from canonical fixtures. This is the main tripwire for regressions when wire data, generated types, or spatial assumptions change.

### Boundary 2: Render model to Three scene graph

Create an imperative scene adapter that takes the render model and populates a `THREE.Group` or `THREE.Scene`:

- `leaf:<id>` groups/meshes for floor planes
- `object:<id>` meshes for smart objects
- `agent:<id>` groups for agent marker + facing indicator
- `userData` metadata on every pickable mesh

Tests inspect the scene graph directly: names, counts, positions, colors, and `userData`. They do not depend on a browser canvas or GPU.

### Boundary 3: Three scene graph to React lifecycle

`WorldScene.tsx` is a thin client component over a small browser runtime adapter:

- owns the `<canvas>` container ref
- asks the runtime adapter to create/dispose `WebGLRenderer`, `Scene`, camera, controls, lights, and root group
- passes render-model updates into the runtime, which rebuilds or patches the root group
- observes size changes so the canvas and camera aspect stay valid
- renders placeholders for `world == null`, `snapshot == null`, or empty scene data

React tests cover placeholders and lifecycle wiring by mocking the browser runtime. WebGL-specific behavior stays behind the runtime and browser-smoke layers, while scene-object correctness stays in the Three scene graph tests.

### Boundary 4: Browser rendering

Automated browser smoke should load the page and assert:

- no console errors
- world-scene canvas exists
- canvas has nonblank pixels
- primary layout is not collapsed

This catches browser-only issues that jsdom cannot see: WebGL context creation, CSS sizing, camera framing, and runtime import failures.

## Testing Strategy

Use TDD for implementation:

1. Write failing Vitest tests for coordinate conversion, leaf projection, phase styling, and stable ordering.
2. Implement the pure render-model layer until those tests pass.
3. Write failing Vitest tests for Three scene graph population.
4. Implement the scene adapter until those tests pass.
5. Write failing React component tests for placeholder and data states.
6. Implement `WorldScene` and page layout changes until those tests pass.
7. Add a browser smoke test if Playwright is available or installable for the web app; otherwise run an in-app browser verification and document the gap.

Full verification for the pass:

```bash
pnpm test
pnpm lint
pnpm build
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`cargo test --workspace` requires permission to bind localhost for the host WebSocket smoke test in the current sandbox.

## Follow-up

The natural next pass is interactive inspection:

- click/pick agent and object meshes
- selected entity panel
- optional `PlayerInput` or request/response shape for full sim-side agent details
- richer explanation of why the agent is doing its current action
