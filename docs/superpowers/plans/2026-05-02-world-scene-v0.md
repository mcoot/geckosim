# World Scene v0 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Three.js observable world scene to the Next.js frontend using the existing `WorldLayout` and `Snapshot` wire data.

**Architecture:** Keep the renderer split into explicit boundaries: generated wire types -> pure render model -> Three scene graph adapter -> thin React lifecycle component. The scene is top-down by default but uses real Three.js camera/mesh primitives so later full-camera and inspection work extend the same surface.

**Tech Stack:** Next.js 16, React 19, TypeScript 5, Tailwind v4, Vitest/jsdom, Three.js, Rust workspace verification via cargo.

**Reference:** Spec at [`docs/superpowers/specs/2026-05-02-world-scene-v0-design.md`](../specs/2026-05-02-world-scene-v0-design.md). Predecessor spatial spec at [`docs/superpowers/specs/2026-04-28-spatial-pass-design.md`](../specs/2026-04-28-spatial-pass-design.md).

**VCS note:** This repo uses `jj` (jujutsu), colocated with git. **Never use raw `git`.** Each task starts from the current working-copy commit with `jj new -m "<task title>"` when creating a new logical change. There is no staging area and no `git commit`.

---

## File Structure

**Create:**
- `apps/web/src/lib/world-scene/model.ts` - pure projection from `WorldLayout + Snapshot` to scene-ready data.
- `apps/web/src/lib/world-scene/model.test.ts` - Vitest tests for projection, stable ordering, phases, and null/empty handling.
- `apps/web/src/lib/world-scene/scene.ts` - Three.js scene graph adapter; no React.
- `apps/web/src/lib/world-scene/scene.test.ts` - Vitest tests inspecting `THREE.Group`/`Mesh` names, positions, colors, and `userData`.
- `apps/web/src/lib/world-scene/runtime.ts` - browser-only Three.js renderer/camera/controls lifecycle adapter.
- `apps/web/src/components/WorldScene.tsx` - React client component that orchestrates the runtime lifecycle and placeholder states.
- `apps/web/src/components/WorldScene.test.tsx` - React tests for placeholders and fixture-backed render state.

**Modify:**
- `apps/web/package.json` and `apps/web/pnpm-lock.yaml` - add `three`; add Playwright only if we choose automated browser smoke in this pass.
- `apps/web/src/app/page.tsx` - place `WorldScene` as the primary dashboard surface above or beside `AgentList`.
- `apps/web/src/app/globals.css` - only if stable canvas sizing or base layout helpers cannot be expressed cleanly with existing Tailwind classes.
- `apps/web/README.md` - update out-of-scope note now that Three.js rendering has landed; document browser smoke command if added.

**Do not modify:**
- `apps/web/src/types/sim/*.ts` - generated files only; no wire changes expected.
- Rust crates - no protocol or sim changes expected.

---

## Chunk 1: Dependency and Render Model

### Task 1: Add Three.js Dependency

**Files:**
- Modify: `apps/web/package.json`
- Modify: `apps/web/pnpm-lock.yaml`

- [ ] **Step 1.1: Start the task change**

```bash
jj new -m "World scene: add Three.js dependency"
```

- [ ] **Step 1.2: Add dependency**

Run:

```bash
cd apps/web
pnpm add three
```

Expected: `three` appears in `dependencies`; lockfile updates. If network is blocked, rerun with sandbox escalation.

- [ ] **Step 1.3: Verify install metadata**

Run:

```bash
cd apps/web
jq '.dependencies.three' package.json
```

Expected: prints the installed version string.

- [ ] **Step 1.4: Check status**

Run:

```bash
jj st
```

Expected: only `apps/web/package.json` and `apps/web/pnpm-lock.yaml` changed.

### Task 2: Render-Model Tests First

**Files:**
- Create: `apps/web/src/lib/world-scene/model.test.ts`
- Create later: `apps/web/src/lib/world-scene/model.ts`

- [ ] **Step 2.1: Start the task change**

```bash
jj new -m "World scene: project wire data into render model"
```

- [ ] **Step 2.2: Write failing projection tests**

Create `apps/web/src/lib/world-scene/model.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { Snapshot } from "@/types/sim/Snapshot";
import type { WorldLayout } from "@/types/sim/WorldLayout";
import {
  buildWorldSceneModel,
  headingRadians,
  phaseStyle,
  simToGround,
} from "./model";

const world: WorldLayout = {
  districts: [{ id: 1, display_name: "Old Town", bbox: { min: { x: 0, y: 0 }, max: { x: 200, y: 200 } } }],
  buildings: [],
  floors: [],
  leaves: [
    {
      id: 3,
      display_name: "Living Room",
      kind: { Room: { building: 1, floor: 1 } },
      bbox: { min: { x: 80, y: 80 }, max: { x: 120, y: 100 } },
      adjacency: [],
    },
    {
      id: 1,
      display_name: "Town Plaza",
      kind: { OutdoorZone: "Plaza" },
      bbox: { min: { x: 0, y: 0 }, max: { x: 80, y: 80 } },
      adjacency: [3],
    },
  ],
  default_spawn_leaf: 3,
};

const snapshot: Snapshot = {
  tick: 12,
  agents: [
    {
      id: 7,
      name: "Ada",
      needs: { hunger: 0.4, sleep: 0.8, social: 1, hygiene: 1, fun: 0.5, comfort: 0.9 },
      mood: { valence: 0.1, arousal: 0.2, stress: 0.3 },
      personality: { openness: 0, conscientiousness: 0, extraversion: 0, agreeableness: 0, neuroticism: 0 },
      leaf: 3,
      pos: { x: 90, y: 84 },
      facing: { x: 0, y: 1 },
      action_phase: "Walking",
      current_action: { display_name: "Eat snack", fraction_complete: 0.25 },
    },
  ],
  objects: [{ id: 2, type_id: 1, leaf: 3, pos: { x: 96, y: 88 } }],
};

describe("world scene model", () => {
  it("maps sim Vec2 to the Three ground plane", () => {
    expect(simToGround({ x: 4, y: -2 })).toEqual({ x: 4, y: 0, z: -2 });
  });

  it("computes heading in radians from a sim facing vector", () => {
    expect(headingRadians({ x: 0, y: 1 })).toBeCloseTo(0);
    expect(headingRadians({ x: 1, y: 0 })).toBeCloseTo(Math.PI / 2);
  });

  it("uses distinct phase styles", () => {
    expect(phaseStyle("Walking").agentColor).not.toBe(phaseStyle("Performing").agentColor);
    expect(phaseStyle(null).agentColor).not.toBe(phaseStyle("Walking").agentColor);
  });

  it("projects world and snapshot into stable scene data", () => {
    const model = buildWorldSceneModel(world, snapshot);
    expect(model.leaves.map((leaf) => leaf.id)).toEqual([1, 3]);
    expect(model.leaves[1]).toMatchObject({
      id: 3,
      name: "Living Room",
      center: { x: 100, y: 0, z: 90 },
      size: { x: 40, z: 20 },
      kind: "room",
    });
    expect(model.objects[0]).toMatchObject({
      id: 2,
      typeId: 1,
      position: { x: 96, y: 0, z: 88 },
      leafId: 3,
    });
    expect(model.agents[0]).toMatchObject({
      id: 7,
      name: "Ada",
      position: { x: 90, y: 0, z: 84 },
      leafId: 3,
      phase: "Walking",
    });
  });

  it("returns empty arrays without throwing when data is missing", () => {
    expect(buildWorldSceneModel(null, null)).toEqual({ leaves: [], objects: [], agents: [], bounds: null });
  });
});
```

- [ ] **Step 2.3: Run the focused test and verify RED**

Run:

```bash
cd apps/web
pnpm test src/lib/world-scene/model.test.ts
```

Expected: fails because `./model` does not exist.

- [ ] **Step 2.4: Implement minimal render model**

Create `apps/web/src/lib/world-scene/model.ts`:

```ts
import type { AgentSnapshot } from "@/types/sim/AgentSnapshot";
import type { LeafArea } from "@/types/sim/LeafArea";
import type { Phase } from "@/types/sim/Phase";
import type { Snapshot } from "@/types/sim/Snapshot";
import type { Vec2 } from "@/types/sim/Vec2";
import type { WorldLayout } from "@/types/sim/WorldLayout";

export interface GroundPoint {
  x: number;
  y: number;
  z: number;
}

export interface LeafRenderModel {
  id: number;
  name: string;
  kind: "room" | "outdoor";
  center: GroundPoint;
  size: { x: number; z: number };
  color: string;
}

export interface ObjectRenderModel {
  id: number;
  typeId: number;
  leafId: number;
  position: GroundPoint;
  color: string;
}

export interface AgentRenderModel {
  id: number;
  name: string;
  leafId: number;
  position: GroundPoint;
  heading: number;
  phase: Phase | null;
  color: string;
}

export interface WorldSceneModel {
  leaves: LeafRenderModel[];
  objects: ObjectRenderModel[];
  agents: AgentRenderModel[];
  bounds: { min: GroundPoint; max: GroundPoint } | null;
}

export const EMPTY_WORLD_SCENE_MODEL: WorldSceneModel = {
  leaves: [],
  objects: [],
  agents: [],
  bounds: null,
};

export function simToGround(pos: Vec2): GroundPoint {
  return { x: pos.x, y: 0, z: pos.y };
}

export function headingRadians(facing: Vec2): number {
  return Math.atan2(facing.x, facing.y);
}

export function phaseStyle(phase: Phase | null): { agentColor: string } {
  if (phase === "Walking") return { agentColor: "#3b82f6" };
  if (phase === "Performing") return { agentColor: "#22c55e" };
  return { agentColor: "#a3a3a3" };
}

function leafKind(leaf: LeafArea): "room" | "outdoor" {
  return "Room" in leaf.kind ? "room" : "outdoor";
}

function leafColor(kind: "room" | "outdoor"): string {
  return kind === "room" ? "#334155" : "#14532d";
}

function projectLeaf(leaf: LeafArea): LeafRenderModel {
  const kind = leafKind(leaf);
  const center = simToGround({
    x: (leaf.bbox.min.x + leaf.bbox.max.x) / 2,
    y: (leaf.bbox.min.y + leaf.bbox.max.y) / 2,
  });
  return {
    id: leaf.id,
    name: leaf.display_name,
    kind,
    center,
    size: {
      x: leaf.bbox.max.x - leaf.bbox.min.x,
      z: leaf.bbox.max.y - leaf.bbox.min.y,
    },
    color: leafColor(kind),
  };
}

function projectAgent(agent: AgentSnapshot): AgentRenderModel {
  return {
    id: agent.id,
    name: agent.name,
    leafId: agent.leaf,
    position: simToGround(agent.pos),
    heading: headingRadians(agent.facing),
    phase: agent.action_phase,
    color: phaseStyle(agent.action_phase).agentColor,
  };
}

export function buildWorldSceneModel(
  world: WorldLayout | null,
  snapshot: Snapshot | null,
): WorldSceneModel {
  if (!world || !snapshot) {
    return EMPTY_WORLD_SCENE_MODEL;
  }

  const leaves = [...world.leaves].sort((a, b) => a.id - b.id).map(projectLeaf);
  const objects = [...snapshot.objects]
    .sort((a, b) => a.id - b.id)
    .map((object) => ({
      id: object.id,
      typeId: object.type_id,
      leafId: object.leaf,
      position: simToGround(object.pos),
      color: "#f59e0b",
    }));
  const agents = [...snapshot.agents].sort((a, b) => a.id - b.id).map(projectAgent);

  return {
    leaves,
    objects,
    agents,
    bounds: leaves.length
      ? {
          min: {
            x: Math.min(...leaves.map((leaf) => leaf.center.x - leaf.size.x / 2)),
            y: 0,
            z: Math.min(...leaves.map((leaf) => leaf.center.z - leaf.size.z / 2)),
          },
          max: {
            x: Math.max(...leaves.map((leaf) => leaf.center.x + leaf.size.x / 2)),
            y: 0,
            z: Math.max(...leaves.map((leaf) => leaf.center.z + leaf.size.z / 2)),
          },
        }
      : null,
  };
}
```

- [ ] **Step 2.5: Run focused test and verify GREEN**

Run:

```bash
cd apps/web
pnpm test src/lib/world-scene/model.test.ts
```

Expected: pass.

---

## Chunk 2: Three Scene Adapter

### Task 3: Scene Graph Tests First

**Files:**
- Create: `apps/web/src/lib/world-scene/scene.test.ts`
- Create later: `apps/web/src/lib/world-scene/scene.ts`

- [ ] **Step 3.1: Start the task change**

```bash
jj new -m "World scene: build Three.js scene graph from render model"
```

- [ ] **Step 3.2: Write failing scene graph tests**

Create `apps/web/src/lib/world-scene/scene.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { populateWorldGroup } from "./scene";
import type { WorldSceneModel } from "./model";

const model: WorldSceneModel = {
  bounds: { min: { x: 0, y: 0, z: 0 }, max: { x: 100, y: 0, z: 100 } },
  leaves: [
    { id: 1, name: "Plaza", kind: "outdoor", center: { x: 40, y: 0, z: 40 }, size: { x: 80, z: 80 }, color: "#14532d" },
  ],
  objects: [
    { id: 2, typeId: 1, leafId: 1, position: { x: 25, y: 0, z: 30 }, color: "#f59e0b" },
  ],
  agents: [
    { id: 7, name: "Ada", leafId: 1, position: { x: 50, y: 0, z: 50 }, heading: Math.PI / 2, phase: "Walking", color: "#3b82f6" },
  ],
};

describe("world scene graph", () => {
  it("creates named groups with stable userData IDs", () => {
    const root = new THREE.Group();
    populateWorldGroup(root, model);

    expect(root.getObjectByName("leaf:1")?.userData).toMatchObject({ leafId: 1, kind: "outdoor" });
    expect(root.getObjectByName("object:2")?.userData).toMatchObject({ objectId: 2, typeId: 1, leafId: 1 });
    expect(root.getObjectByName("agent:7")?.userData).toMatchObject({ agentId: 7, leafId: 1, phase: "Walking" });
  });

  it("places model objects in Three coordinates", () => {
    const root = new THREE.Group();
    populateWorldGroup(root, model);

    expect(root.getObjectByName("object:2")?.position.toArray()).toEqual([25, 0.7, 30]);
    expect(root.getObjectByName("agent:7")?.position.toArray()).toEqual([50, 1, 50]);
    expect(root.getObjectByName("agent:7")?.rotation.y).toBeCloseTo(Math.PI / 2);
  });

  it("clears stale children before repopulating", () => {
    const root = new THREE.Group();
    populateWorldGroup(root, model);
    populateWorldGroup(root, { ...model, agents: [], objects: [] });

    expect(root.getObjectByName("agent:7")).toBeUndefined();
    expect(root.getObjectByName("object:2")).toBeUndefined();
    expect(root.getObjectByName("leaf:1")).toBeDefined();
  });
});
```

- [ ] **Step 3.3: Run focused test and verify RED**

Run:

```bash
cd apps/web
pnpm test src/lib/world-scene/scene.test.ts
```

Expected: fails because `./scene` does not exist.

- [ ] **Step 3.4: Implement minimal scene adapter**

Create `apps/web/src/lib/world-scene/scene.ts`:

```ts
import * as THREE from "three";
import type { AgentRenderModel, LeafRenderModel, ObjectRenderModel, WorldSceneModel } from "./model";

function material(color: string): THREE.MeshStandardMaterial {
  return new THREE.MeshStandardMaterial({ color, roughness: 0.85, metalness: 0 });
}

function disposeObject(object: THREE.Object3D): void {
  object.traverse((child) => {
    if (child instanceof THREE.Mesh) {
      child.geometry.dispose();
      const mats = Array.isArray(child.material) ? child.material : [child.material];
      for (const mat of mats) mat.dispose();
    }
  });
}

function addLeaf(root: THREE.Group, leaf: LeafRenderModel): void {
  const mesh = new THREE.Mesh(
    new THREE.PlaneGeometry(leaf.size.x, leaf.size.z),
    material(leaf.color),
  );
  mesh.name = `leaf:${leaf.id}`;
  mesh.position.set(leaf.center.x, -0.02, leaf.center.z);
  mesh.rotation.x = -Math.PI / 2;
  mesh.userData = { leafId: leaf.id, kind: leaf.kind, name: leaf.name };
  root.add(mesh);
}

function addObject(root: THREE.Group, object: ObjectRenderModel): void {
  const mesh = new THREE.Mesh(new THREE.BoxGeometry(1.2, 1.2, 1.2), material(object.color));
  mesh.name = `object:${object.id}`;
  mesh.position.set(object.position.x, 0.7, object.position.z);
  mesh.userData = { objectId: object.id, typeId: object.typeId, leafId: object.leafId };
  root.add(mesh);
}

function addAgent(root: THREE.Group, agent: AgentRenderModel): void {
  const group = new THREE.Group();
  group.name = `agent:${agent.id}`;
  group.position.set(agent.position.x, 1, agent.position.z);
  group.rotation.y = agent.heading;
  group.userData = { agentId: agent.id, leafId: agent.leafId, phase: agent.phase, name: agent.name };

  const body = new THREE.Mesh(new THREE.ConeGeometry(1.4, 2.4, 24), material(agent.color));
  body.rotation.x = Math.PI / 2;
  body.userData = group.userData;
  group.add(body);

  root.add(group);
}

export function populateWorldGroup(root: THREE.Group, model: WorldSceneModel): void {
  for (const child of [...root.children]) {
    root.remove(child);
    disposeObject(child);
  }

  for (const leaf of model.leaves) addLeaf(root, leaf);
  for (const object of model.objects) addObject(root, object);
  for (const agent of model.agents) addAgent(root, agent);
}
```

- [ ] **Step 3.5: Run focused test and verify GREEN**

Run:

```bash
cd apps/web
pnpm test src/lib/world-scene/scene.test.ts
```

Expected: pass.

---

## Chunk 3: React Component and Page Integration

### Task 4: Component Tests First

**Files:**
- Create: `apps/web/src/components/WorldScene.test.tsx`
- Create later: `apps/web/src/components/WorldScene.tsx`
- Create later: `apps/web/src/lib/world-scene/runtime.ts`

- [ ] **Step 4.1: Start the task change**

```bash
jj new -m "World scene: add React canvas component"
```

- [ ] **Step 4.2: Write failing component tests**

Create `apps/web/src/components/WorldScene.test.tsx`:

```tsx
import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Snapshot } from "@/types/sim/Snapshot";
import type { WorldLayout } from "@/types/sim/WorldLayout";
import { WorldScene } from "./WorldScene";

const runtimeMock = vi.hoisted(() => ({
  createWorldSceneRuntime: vi.fn(() => ({
    update: vi.fn(),
    dispose: vi.fn(),
  })),
}));

vi.mock("@/lib/world-scene/runtime", () => runtimeMock);

const world: WorldLayout = {
  districts: [],
  buildings: [],
  floors: [],
  leaves: [
    {
      id: 1,
      display_name: "Town Plaza",
      kind: { OutdoorZone: "Plaza" },
      bbox: { min: { x: 0, y: 0 }, max: { x: 80, y: 80 } },
      adjacency: [],
    },
  ],
  default_spawn_leaf: 1,
};

const snapshot: Snapshot = { tick: 1, agents: [], objects: [] };

describe("WorldScene", () => {
  beforeEach(() => {
    runtimeMock.createWorldSceneRuntime.mockClear();
  });

  it("shows a placeholder before init data arrives", () => {
    render(<WorldScene world={null} snapshot={null} />);
    expect(screen.getByText("Waiting for world data")).toBeInTheDocument();
  });

  it("shows the Three.js scene container when data is available", () => {
    render(<WorldScene world={world} snapshot={snapshot} />);
    expect(screen.getByLabelText("World scene")).toBeInTheDocument();
    expect(screen.getByText("tick 1")).toBeInTheDocument();
    expect(runtimeMock.createWorldSceneRuntime).toHaveBeenCalled();
  });
});
```

- [ ] **Step 4.3: Run focused test and verify RED**

Run:

```bash
cd apps/web
pnpm test src/components/WorldScene.test.tsx
```

Expected: fails because `WorldScene` does not exist.

- [ ] **Step 4.4: Implement minimal component**

Create `apps/web/src/lib/world-scene/runtime.ts`:

```ts
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { WorldSceneModel } from "./model";
import { populateWorldGroup } from "./scene";

export interface WorldSceneRuntime {
  update: (model: WorldSceneModel) => void;
  dispose: () => void;
}

export function createWorldSceneRuntime(
  mount: HTMLElement,
  initialModel: WorldSceneModel,
): WorldSceneRuntime {
  const renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  mount.appendChild(renderer.domElement);

  const scene = new THREE.Scene();
  scene.background = new THREE.Color("#0a0a0a");
  const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 1000);
  camera.position.set(80, 140, 140);
  camera.lookAt(60, 0, 60);

  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.target.set(60, 0, 60);

  scene.add(new THREE.HemisphereLight("#ffffff", "#334155", 2));
  const root = new THREE.Group();
  root.name = "world-root";
  scene.add(root);

  const resize = () => {
    const width = mount.clientWidth || 640;
    const height = mount.clientHeight || 360;
    renderer.setSize(width, height);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
  };

  const observer = new ResizeObserver(resize);
  observer.observe(mount);
  resize();

  let frame = 0;
  const render = () => {
    frame = window.requestAnimationFrame(render);
    controls.update();
    renderer.render(scene, camera);
  };
  render();

  const update = (model: WorldSceneModel) => {
    populateWorldGroup(root, model);
  };
  update(initialModel);

  return {
    update,
    dispose: () => {
      observer.disconnect();
      window.cancelAnimationFrame(frame);
      controls.dispose();
      renderer.dispose();
      renderer.domElement.remove();
    },
  };
}
```

Create `apps/web/src/components/WorldScene.tsx`:

```tsx
"use client";

import { useEffect, useMemo, useRef } from "react";
import type { Snapshot } from "@/types/sim/Snapshot";
import type { WorldLayout } from "@/types/sim/WorldLayout";
import {
  buildWorldSceneModel,
  EMPTY_WORLD_SCENE_MODEL,
} from "@/lib/world-scene/model";
import {
  createWorldSceneRuntime,
  type WorldSceneRuntime,
} from "@/lib/world-scene/runtime";

interface WorldSceneProps {
  world: WorldLayout | null;
  snapshot: Snapshot | null;
}

export function WorldScene({ world, snapshot }: WorldSceneProps) {
  const mountRef = useRef<HTMLDivElement | null>(null);
  const runtimeRef = useRef<WorldSceneRuntime | null>(null);
  const model = useMemo(() => buildWorldSceneModel(world, snapshot), [world, snapshot]);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return;

    runtimeRef.current = createWorldSceneRuntime(mount, EMPTY_WORLD_SCENE_MODEL);

    return () => {
      runtimeRef.current?.dispose();
      runtimeRef.current = null;
    };
  }, []);

  useEffect(() => {
    runtimeRef.current?.update(model);
  }, [model]);

  return (
    <section aria-label="World scene" className="space-y-2">
      <div
        ref={mountRef}
        className="min-h-96 overflow-hidden rounded-lg border border-neutral-300 bg-neutral-950 dark:border-neutral-700"
      />
      {!world || !snapshot ? (
        <p className="text-sm text-neutral-500">Waiting for world data</p>
      ) : (
        <p className="text-xs text-neutral-500">
          tick {snapshot.tick} · {model.leaves.length} leaves · {model.agents.length} agents ·{" "}
          {model.objects.length} objects
        </p>
      )}
    </section>
  );
}
```

Note for implementation: setup should happen once per data-availability lifetime. Model changes should call `runtime.update(model)`, not recreate the renderer on every tick.

- [ ] **Step 4.5: Run focused test and verify GREEN**

Run:

```bash
cd apps/web
pnpm test src/components/WorldScene.test.tsx
```

Expected: pass. The runtime is mocked so jsdom does not need a real WebGL context.

### Task 5: Integrate Scene Into Dashboard

**Files:**
- Modify: `apps/web/src/app/page.tsx`
- Modify: `apps/web/src/components/WorldScene.tsx`

- [ ] **Step 5.1: Write failing page integration test or extend component test**

If no page tests exist, extend `WorldScene.test.tsx` to assert the component accepts real `world`/`snapshot` props and renders scene stats. This may already be covered by Task 4; if so, no extra test is needed.

- [ ] **Step 5.2: Update `page.tsx`**

Change the dashboard to import and render `WorldScene` inside the existing provider through a small child component:

```tsx
"use client";

import { AgentList } from "@/components/AgentList";
import { ConnectionStatus } from "@/components/ConnectionStatus";
import { Controls } from "@/components/Controls";
import { WorldScene } from "@/components/WorldScene";
import { SimConnectionProvider, useSimConnection } from "@/lib/sim/connection";

function Dashboard() {
  const { state } = useSimConnection();
  return (
    <main className="mx-auto flex w-full max-w-6xl flex-1 flex-col gap-4 p-6">
      <header className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">gecko-sim</h1>
        <ConnectionStatus />
      </header>
      <Controls />
      <WorldScene world={state.world} snapshot={state.snapshot} />
      <div className="overflow-x-auto">
        <AgentList />
      </div>
    </main>
  );
}

export default function Page() {
  return (
    <SimConnectionProvider>
      <Dashboard />
    </SimConnectionProvider>
  );
}
```

- [ ] **Step 5.3: Run frontend tests**

Run:

```bash
cd apps/web
pnpm test
```

Expected: all Vitest tests pass.

---

## Chunk 4: Verification, Browser Smoke, and Docs

### Task 6: Browser Smoke Strategy

**Files:**
- Optional create: `apps/web/e2e/world-scene.spec.ts`
- Optional modify: `apps/web/package.json`
- Optional modify: `apps/web/pnpm-lock.yaml`
- Modify: `apps/web/README.md`

- [ ] **Step 6.1: Decide whether to add Playwright now**

If `@playwright/test` is already available or the user approves installing it, add a smoke test. Otherwise document manual in-app browser verification in the final notes and keep this pass at Vitest + build verification.

- [ ] **Step 6.2: If adding Playwright, install it**

Run:

```bash
cd apps/web
pnpm add -D @playwright/test
```

Add script:

```json
"test:e2e": "playwright test"
```

- [ ] **Step 6.3: Write a browser smoke test**

Create `apps/web/e2e/world-scene.spec.ts`:

```ts
import { expect, test } from "@playwright/test";

test("world scene canvas renders without console errors", async ({ page }) => {
  const errors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") errors.push(msg.text());
  });

  await page.goto("/");
  await expect(page.getByLabel("World scene")).toBeVisible();
  const canvas = page.locator("canvas").first();
  await expect(canvas).toBeVisible();
  expect(errors).toEqual([]);
});
```

Implementation note: this simple test only works if the host is running and has sent world data. If that makes CI too brittle, defer Playwright until fixture injection exists.

### Task 7: Final Verification

**Files:**
- Modify: `apps/web/README.md`

- [ ] **Step 7.1: Update README out-of-scope section**

Replace the old "Three.js / 3D rendering" out-of-scope note with a statement that v0 rendering exists and future work includes inspection, full free-camera UX, interpolation, and authored meshes.

- [ ] **Step 7.2: Run frontend verification**

Run:

```bash
cd apps/web
pnpm test
pnpm lint
pnpm build
```

Expected: all pass. `pnpm build` may need network access if Next font fetching is still configured.

- [ ] **Step 7.3: Run Rust verification**

Run from repo root:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all pass. `cargo test --workspace` may need sandbox escalation for the host WebSocket smoke test binding localhost.

- [ ] **Step 7.4: Run browser verification**

Run the host:

```bash
cargo run -p gecko-sim-host
```

Run the frontend:

```bash
cd apps/web
pnpm dev
```

Open `http://localhost:3000` in the in-app browser. Verify:

- canvas is visible and nonblank
- leaf planes are framed in the default camera
- agent and object markers are visible
- no browser console errors
- controls and `AgentList` still work

- [ ] **Step 7.5: Inspect jj status and final diff**

Run:

```bash
jj st
jj diff
```

Expected: changes match this plan; no generated type files or Rust files changed unless a deliberate later decision required it.
