import { describe, expect, it } from "vitest";
import * as THREE from "three";
import type { WorldSceneModel } from "./model";
import { populateWorldGroup } from "./scene";

const model: WorldSceneModel = {
  bounds: { min: { x: 0, y: 0, z: 0 }, max: { x: 100, y: 0, z: 100 } },
  leaves: [
    {
      id: 1,
      name: "Plaza",
      kind: "outdoor",
      center: { x: 40, y: 0, z: 40 },
      size: { x: 80, z: 80 },
      color: "#14532d",
    },
  ],
  objects: [
    {
      id: 2,
      typeId: 1,
      leafId: 1,
      position: { x: 25, y: 0, z: 30 },
      color: "#f59e0b",
    },
  ],
  agents: [
    {
      id: 7,
      name: "Ada",
      leafId: 1,
      position: { x: 50, y: 0, z: 50 },
      heading: Math.PI / 2,
      phase: "Walking",
      color: "#3b82f6",
      intent: {
        actionLabel: "Eat snack",
        phase: "Walking",
        progress: 0,
        targetObjectId: 2,
        targetPosition: { x: 25, y: 0, z: 30 },
        targetLabel: "Fridge",
      },
    },
  ],
};

describe("world scene graph", () => {
  it("creates named groups with stable userData IDs", () => {
    const root = new THREE.Group();

    populateWorldGroup(root, model);

    expect(root.getObjectByName("leaf:1")?.userData).toMatchObject({
      leafId: 1,
      kind: "outdoor",
    });
    expect(root.getObjectByName("object:2")?.userData).toMatchObject({
      objectId: 2,
      typeId: 1,
      leafId: 1,
    });
    expect(root.getObjectByName("agent:7")?.userData).toMatchObject({
      agentId: 7,
      leafId: 1,
      phase: "Walking",
    });
  });

  it("places model objects in Three coordinates", () => {
    const root = new THREE.Group();

    populateWorldGroup(root, model);

    expect(root.getObjectByName("object:2")?.position.toArray()).toEqual([
      25, 0.7, 30,
    ]);
    expect(root.getObjectByName("agent:7")?.position.toArray()).toEqual([
      50, 1, 50,
    ]);
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

  it("draws a faint route for walking agents with targets", () => {
    const root = new THREE.Group();

    populateWorldGroup(root, model, { selectedAgentId: null });

    const route = root.getObjectByName("intent-route:7");
    expect(route).toBeDefined();
    expect(route?.userData).toMatchObject({
      agentId: 7,
      targetObjectId: 2,
      kind: "intent-route",
      selected: false,
    });
  });

  it("draws selected route and target marker for the selected agent", () => {
    const root = new THREE.Group();

    populateWorldGroup(root, model, { selectedAgentId: 7 });

    expect(root.getObjectByName("intent-route:7")?.userData).toMatchObject({
      selected: true,
    });
    expect(root.getObjectByName("intent-target:7")?.userData).toMatchObject({
      agentId: 7,
      targetObjectId: 2,
      kind: "intent-target",
    });
  });

  it("does not draw faint routes for non-walking agents", () => {
    const root = new THREE.Group();
    const performing = {
      ...model,
      agents: [
        {
          ...model.agents[0],
          phase: "Performing" as const,
          intent: model.agents[0].intent
            ? { ...model.agents[0].intent, phase: "Performing" as const }
            : null,
        },
      ],
    };

    populateWorldGroup(root, performing, { selectedAgentId: null });

    expect(root.getObjectByName("intent-route:7")).toBeUndefined();
  });
});
