import "@testing-library/jest-dom/vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Snapshot } from "@/types/sim/Snapshot";
import type { WorldLayout } from "@/types/sim/WorldLayout";
import { WorldScene } from "./WorldScene";

const runtimeMock = vi.hoisted(() => {
  const mock = {
    canCreateWorldSceneRuntime: vi.fn(() => true),
    latestOnSelectAgent: null as null | ((agentId: number) => void),
    latestUpdate: null as null | ReturnType<typeof vi.fn>,
    createWorldSceneRuntime: vi.fn(
      (
        _mount: HTMLElement,
        _model: unknown,
        options?: { onSelectAgent?: (agentId: number) => void },
      ) => {
        mock.latestOnSelectAgent = options?.onSelectAgent ?? null;
        mock.latestUpdate = vi.fn();
        return {
          update: mock.latestUpdate,
          dispose: vi.fn(),
        };
      },
    ),
  };
  return mock;
});

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
const walkingSnapshot: Snapshot = {
  tick: 2,
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
      leaf: 1,
      pos: { x: 10, y: 12 },
      facing: { x: 0, y: 1 },
      action_phase: "Walking",
      current_action: {
        display_name: "Eat snack",
        fraction_complete: 0.25,
        phase: "Walking",
        target_object_id: 2,
        target_position: { x: 25, y: 30 },
        target_label: "Fridge",
      },
    },
  ],
  objects: [{ id: 2, type_id: 1, leaf: 1, pos: { x: 25, y: 30 } }],
};

describe("WorldScene", () => {
  beforeEach(() => {
    runtimeMock.canCreateWorldSceneRuntime.mockClear();
    runtimeMock.canCreateWorldSceneRuntime.mockReturnValue(true);
    runtimeMock.createWorldSceneRuntime.mockClear();
    runtimeMock.latestOnSelectAgent = null;
    runtimeMock.latestUpdate = null;
    runtimeMock.createWorldSceneRuntime.mockImplementation(
      (
        _mount: HTMLElement,
        _model: unknown,
        options?: { onSelectAgent?: (agentId: number) => void },
      ) => {
        runtimeMock.latestOnSelectAgent = options?.onSelectAgent ?? null;
        runtimeMock.latestUpdate = vi.fn();
        return {
          update: runtimeMock.latestUpdate,
          dispose: vi.fn(),
        };
      },
    );
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

  it("passes selected gecko id into the runtime update", async () => {
    render(
      <WorldScene
        world={world}
        snapshot={walkingSnapshot}
        selectedAgentId={7}
        onSelectAgent={vi.fn()}
      />,
    );

    await waitFor(() =>
      expect(runtimeMock.latestUpdate).toHaveBeenLastCalledWith(
        expect.anything(),
        7,
      ),
    );
  });

  it("notifies the page when a gecko is selected in the scene", () => {
    const onSelectAgent = vi.fn();
    render(
      <WorldScene
        world={world}
        snapshot={walkingSnapshot}
        selectedAgentId={null}
        onSelectAgent={onSelectAgent}
      />,
    );

    act(() => runtimeMock.latestOnSelectAgent?.(7));

    expect(onSelectAgent).toHaveBeenCalledWith(7);
  });

  it("clears the runtime highlight when the selected gecko disappears", async () => {
    const { rerender } = render(
      <WorldScene
        world={world}
        snapshot={walkingSnapshot}
        selectedAgentId={7}
        onSelectAgent={vi.fn()}
      />,
    );

    await waitFor(() =>
      expect(runtimeMock.latestUpdate).toHaveBeenLastCalledWith(
        expect.anything(),
        7,
      ),
    );

    rerender(
      <WorldScene
        world={world}
        snapshot={snapshot}
        selectedAgentId={7}
        onSelectAgent={vi.fn()}
      />,
    );

    await waitFor(() =>
      expect(runtimeMock.latestUpdate).toHaveBeenLastCalledWith(
        expect.anything(),
        null,
      ),
    );
  });
});
