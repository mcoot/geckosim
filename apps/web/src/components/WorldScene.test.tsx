import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Snapshot } from "@/types/sim/Snapshot";
import type { WorldLayout } from "@/types/sim/WorldLayout";
import { WorldScene } from "./WorldScene";

const runtimeMock = vi.hoisted(() => ({
  canCreateWorldSceneRuntime: vi.fn(() => true),
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
    runtimeMock.canCreateWorldSceneRuntime.mockClear();
    runtimeMock.canCreateWorldSceneRuntime.mockReturnValue(true);
    runtimeMock.createWorldSceneRuntime.mockClear();
    runtimeMock.createWorldSceneRuntime.mockImplementation(() => ({
      update: vi.fn(),
      dispose: vi.fn(),
    }));
  });

  it("shows a placeholder before init data arrives", () => {
    render(<WorldScene world={null} snapshot={null} />);

    expect(screen.getByLabelText("World scene")).toBeInTheDocument();
    expect(screen.getByText("Waiting for world data")).toBeInTheDocument();
  });

  it("shows the Three.js scene container when data is available", () => {
    render(<WorldScene world={world} snapshot={snapshot} />);

    expect(screen.getByLabelText("World scene")).toBeInTheDocument();
    expect(screen.getByText(/tick 1/)).toBeInTheDocument();
    expect(runtimeMock.createWorldSceneRuntime).toHaveBeenCalled();
  });

  it("shows a fallback when WebGL runtime creation fails", async () => {
    runtimeMock.createWorldSceneRuntime.mockImplementation(() => {
      throw new Error("WebGL unavailable");
    });

    expect(() => render(<WorldScene world={world} snapshot={snapshot} />)).not.toThrow();

    expect(await screen.findByText("3D view unavailable")).toBeInTheDocument();
    expect(screen.getByText(/tick 1/)).toBeInTheDocument();
  });

  it("does not create the runtime when WebGL is unavailable", async () => {
    runtimeMock.canCreateWorldSceneRuntime.mockReturnValue(false);

    render(<WorldScene world={world} snapshot={snapshot} />);

    expect(await screen.findByText("3D view unavailable")).toBeInTheDocument();
    expect(runtimeMock.createWorldSceneRuntime).not.toHaveBeenCalled();
  });
});
