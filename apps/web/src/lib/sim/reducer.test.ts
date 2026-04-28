import { describe, expect, it } from "vitest";
import { initialState, reduce, type SimState } from "./reducer";
import type { Snapshot } from "@/types/sim/Snapshot";
import type { WorldLayout } from "@/types/sim/WorldLayout";

const fixtureSnapshot = (tick: number): Snapshot => ({
  tick,
  agents: [
    {
      id: 0,
      name: "Alice",
      needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
      mood: { valence: 0, arousal: 0, stress: 0 },
      personality: {
        openness: 0,
        conscientiousness: 0,
        extraversion: 0,
        agreeableness: 0,
        neuroticism: 0,
      },
      leaf: 1,
      pos: { x: 0, y: 0 },
      facing: { x: 1, y: 0 },
      action_phase: null,
      current_action: null,
    },
  ],
  objects: [],
});

const fixtureWorld = (): WorldLayout => ({
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
      id: 1,
      display_name: "Plaza",
      kind: { OutdoorZone: "Plaza" },
      bbox: { min: { x: 0, y: 0 }, max: { x: 80, y: 80 } },
      adjacency: [],
    },
  ],
  default_spawn_leaf: 1,
});

describe("reduce", () => {
  it("hello message leaves state unchanged (still connecting)", () => {
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "hello", protocol_version: 1, format: "json" },
    });
    expect(next).toEqual(initialState);
  });

  it("init message transitions to connected and stores snapshot", () => {
    const snap = fixtureSnapshot(0);
    const world = fixtureWorld();
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 0, world, snapshot: snap },
    });
    expect(next.status).toBe("connected");
    expect(next.snapshot).toBe(snap);
    expect(next.world).toBe(world);
    expect(next.lastTick).toBe(0);
  });

  it("snapshot message replaces snapshot and updates lastTick", () => {
    const initSnap = fixtureSnapshot(0);
    const connected: SimState = {
      status: "connected",
      snapshot: initSnap,
      world: fixtureWorld(),
      lastTick: 0,
    };
    const newSnap = fixtureSnapshot(5);
    const next = reduce(connected, {
      kind: "server-message",
      msg: { type: "snapshot", snapshot: newSnap },
    });
    expect(next.snapshot).toBe(newSnap);
    expect(next.lastTick).toBe(5);
    expect(next.status).toBe("connected");
    // World retained from prior state.
    expect(next.world).toBe(connected.world);
  });

  it("ws-close transitions to disconnected, retains snapshot", () => {
    const initSnap = fixtureSnapshot(3);
    const connected: SimState = {
      status: "connected",
      snapshot: initSnap,
      world: fixtureWorld(),
      lastTick: 3,
    };
    const next = reduce(connected, { kind: "ws-close" });
    expect(next.status).toBe("disconnected");
    expect(next.snapshot).toBe(initSnap);
  });

  it("ws-error transitions to disconnected", () => {
    const next = reduce(initialState, { kind: "ws-error" });
    expect(next.status).toBe("disconnected");
  });

  it("init message preserves the mood field on the snapshot", () => {
    const snap: Snapshot = {
      tick: 10,
      agents: [
        {
          id: 0,
          name: "Alice",
          needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
          mood: { valence: -0.5, arousal: 0.7, stress: 0.3 },
          personality: {
            openness: 0,
            conscientiousness: 0,
            extraversion: 0,
            agreeableness: 0,
            neuroticism: 0,
          },
          leaf: 1,
          pos: { x: 0, y: 0 },
          facing: { x: 1, y: 0 },
          action_phase: null,
          current_action: null,
        },
      ],
      objects: [],
    };
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 10, world: fixtureWorld(), snapshot: snap },
    });
    expect(next.snapshot?.agents[0].mood).toEqual({
      valence: -0.5,
      arousal: 0.7,
      stress: 0.3,
    });
  });

  it("init message preserves the current_action field on the snapshot", () => {
    const snap: Snapshot = {
      tick: 10,
      agents: [
        {
          id: 0,
          name: "Alice",
          needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
          mood: { valence: 0, arousal: 0, stress: 0 },
          personality: {
            openness: 0,
            conscientiousness: 0,
            extraversion: 0,
            agreeableness: 0,
            neuroticism: 0,
          },
          leaf: 1,
          pos: { x: 0, y: 0 },
          facing: { x: 1, y: 0 },
          action_phase: "Performing",
          current_action: { display_name: "Eat snack", fraction_complete: 0.5 },
        },
      ],
      objects: [],
    };
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 10, world: fixtureWorld(), snapshot: snap },
    });
    expect(next.snapshot?.agents[0].current_action).toEqual({
      display_name: "Eat snack",
      fraction_complete: 0.5,
    });
  });

  it("init message preserves the personality field on the snapshot", () => {
    const snap: Snapshot = {
      tick: 5,
      agents: [
        {
          id: 0,
          name: "Alice",
          needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
          mood: { valence: 0, arousal: 0, stress: 0 },
          personality: {
            openness: 0.4,
            conscientiousness: -0.3,
            extraversion: 0.7,
            agreeableness: -0.5,
            neuroticism: 0.1,
          },
          leaf: 1,
          pos: { x: 0, y: 0 },
          facing: { x: 1, y: 0 },
          action_phase: null,
          current_action: null,
        },
      ],
      objects: [],
    };
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 5, world: fixtureWorld(), snapshot: snap },
    });
    expect(next.snapshot?.agents[0].personality).toEqual({
      openness: 0.4,
      conscientiousness: -0.3,
      extraversion: 0.7,
      agreeableness: -0.5,
      neuroticism: 0.1,
    });
  });

  it("init message stores the world layout", () => {
    const world = fixtureWorld();
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 0, world, snapshot: fixtureSnapshot(0) },
    });
    expect(next.world).toEqual(world);
    expect(next.world?.leaves.length).toBe(1);
    expect(next.world?.default_spawn_leaf).toBe(1);
  });

  it("init message preserves spatial fields on the agent snapshot", () => {
    const snap: Snapshot = {
      tick: 7,
      agents: [
        {
          id: 0,
          name: "Alice",
          needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
          mood: { valence: 0, arousal: 0, stress: 0 },
          personality: {
            openness: 0,
            conscientiousness: 0,
            extraversion: 0,
            agreeableness: 0,
            neuroticism: 0,
          },
          leaf: 3,
          pos: { x: 12.5, y: -3.25 },
          facing: { x: 0, y: 1 },
          action_phase: "Walking",
          current_action: null,
        },
      ],
      objects: [
        { id: 0, type_id: 1, leaf: 3, pos: { x: 15, y: 0 } },
      ],
    };
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 7, world: fixtureWorld(), snapshot: snap },
    });
    expect(next.snapshot?.agents[0].leaf).toBe(3);
    expect(next.snapshot?.agents[0].pos).toEqual({ x: 12.5, y: -3.25 });
    expect(next.snapshot?.agents[0].action_phase).toBe("Walking");
    expect(next.snapshot?.objects.length).toBe(1);
  });
});
