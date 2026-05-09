import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SimState } from "@/lib/sim/reducer";
import type { Snapshot } from "@/types/sim/Snapshot";
import type { WorldLayout } from "@/types/sim/WorldLayout";
import { Dashboard } from "./page";

const world: WorldLayout = {
  districts: [],
  buildings: [],
  floors: [
    {
      id: 1,
      building: 1,
      level: 0,
    },
  ],
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

const snapshot: Snapshot = {
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

const connectionMock = vi.hoisted(() => ({
  state: null as SimState | null,
  sendInput: vi.fn(),
}));

vi.mock("@/lib/sim/connection", () => ({
  useSimConnection: () => ({
    state: connectionMock.state,
    sendInput: connectionMock.sendInput,
  }),
  SimConnectionProvider: ({ children }: { children: ReactNode }) => children,
}));

vi.mock("@/components/ConnectionStatus", () => ({
  ConnectionStatus: () => <span>connected</span>,
}));

vi.mock("@/components/Controls", () => ({
  Controls: () => <section>controls</section>,
}));

vi.mock("@/components/WorldScene", () => ({
  WorldScene: ({
    selectedAgentId,
    onSelectAgent,
  }: {
    selectedAgentId: number | null;
    onSelectAgent: (agentId: number) => void;
  }) => (
    <section aria-label="World scene">
      <button type="button" onClick={() => onSelectAgent(7)}>
        Select Ada
      </button>
      <span data-testid="selected-agent-id">{selectedAgentId ?? "none"}</span>
    </section>
  ),
}));

describe("Dashboard", () => {
  beforeEach(() => {
    connectionMock.state = {
      status: "connected",
      snapshot,
      world,
      lastTick: 2,
    };
    connectionMock.sendInput.mockClear();
  });

  it("replaces the needs table with a selected-gecko inspector", () => {
    render(<Dashboard />);

    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.getByText("Select a gecko")).toBeInTheDocument();
    expect(screen.getByTestId("selected-agent-id")).toHaveTextContent("none");

    fireEvent.click(screen.getByRole("button", { name: "Select Ada" }));

    expect(screen.getByRole("heading", { name: "Ada" })).toBeInTheDocument();
    expect(screen.getByText("Walking to Eat snack")).toBeInTheDocument();
    expect(screen.getByTestId("selected-agent-id")).toHaveTextContent("7");
  });

  it("drops the scene highlight when the selected gecko is no longer present", () => {
    const { rerender } = render(<Dashboard />);

    fireEvent.click(screen.getByRole("button", { name: "Select Ada" }));
    expect(screen.getByTestId("selected-agent-id")).toHaveTextContent("7");

    connectionMock.state = {
      status: "connected",
      snapshot: { ...snapshot, agents: [] },
      world,
      lastTick: 3,
    };
    rerender(<Dashboard />);

    expect(screen.getByText("Select a gecko")).toBeInTheDocument();
    expect(screen.getByTestId("selected-agent-id")).toHaveTextContent("none");
  });
});
