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
  districts: [
    {
      id: 1,
      display_name: "Old Town",
      bbox: { min: { x: 0, y: 0 }, max: { x: 200, y: 200 } },
    },
  ],
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
      needs: {
        hunger: 0.4,
        sleep: 0.8,
        social: 1,
        hygiene: 1,
        fun: 0.5,
        comfort: 0.9,
      },
      mood: { valence: 0.1, arousal: 0.2, stress: 0.3 },
      personality: {
        openness: 0,
        conscientiousness: 0,
        extraversion: 0,
        agreeableness: 0,
        neuroticism: 0,
      },
      leaf: 3,
      pos: { x: 90, y: 84 },
      facing: { x: 0, y: 1 },
      action_phase: "Walking",
      current_action: {
        display_name: "Eat snack",
        fraction_complete: 0.25,
        phase: "Walking",
        target_object_id: 2,
        target_position: { x: 96, y: 87 },
        target_label: "Fridge",
      },
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
    expect(phaseStyle("Walking").agentColor).not.toBe(
      phaseStyle("Performing").agentColor,
    );
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
    expect(model.agents[0].intent).toMatchObject({
      actionLabel: "Eat snack",
      phase: "Walking",
      progress: 0.25,
      targetObjectId: 2,
      targetLabel: "Fridge",
      targetPosition: { x: 96, y: 0, z: 87 },
    });
  });

  it("returns empty arrays without throwing when data is missing", () => {
    expect(buildWorldSceneModel(null, null)).toEqual({
      leaves: [],
      objects: [],
      agents: [],
      bounds: null,
    });
  });

  it("omits intent when the agent has no current action", () => {
    const noAction: Snapshot = {
      ...snapshot,
      agents: [
        {
          ...snapshot.agents[0],
          action_phase: null,
          current_action: null,
        },
      ],
    };

    expect(buildWorldSceneModel(world, noAction).agents[0].intent).toBeNull();
  });
});
