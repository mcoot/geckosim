import { describe, expect, it } from "vitest";
import { initialState, reduce, type SimState } from "./reducer";
import type { Snapshot } from "@/types/sim/Snapshot";

const fixtureSnapshot = (tick: number): Snapshot => ({
  tick,
  agents: [
    {
      id: 0,
      name: "Alice",
      needs: { hunger: 1, sleep: 1, social: 1, hygiene: 1, fun: 1, comfort: 1 },
      mood: { valence: 0, arousal: 0, stress: 0 },
      current_action: null,
    },
  ],
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
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 0, snapshot: snap },
    });
    expect(next.status).toBe("connected");
    expect(next.snapshot).toBe(snap);
    expect(next.lastTick).toBe(0);
  });

  it("snapshot message replaces snapshot and updates lastTick", () => {
    const initSnap = fixtureSnapshot(0);
    const connected: SimState = {
      status: "connected",
      snapshot: initSnap,
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
  });

  it("ws-close transitions to disconnected, retains snapshot", () => {
    const initSnap = fixtureSnapshot(3);
    const connected: SimState = {
      status: "connected",
      snapshot: initSnap,
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
          current_action: null,
        },
      ],
    };
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 10, snapshot: snap },
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
          current_action: { display_name: "Eat snack", fraction_complete: 0.5 },
        },
      ],
    };
    const next = reduce(initialState, {
      kind: "server-message",
      msg: { type: "init", current_tick: 10, snapshot: snap },
    });
    expect(next.snapshot?.agents[0].current_action).toEqual({
      display_name: "Eat snack",
      fraction_complete: 0.5,
    });
  });
});
