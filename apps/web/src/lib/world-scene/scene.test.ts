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
});
