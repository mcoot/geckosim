import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { AgentSnapshot } from "@/types/sim/AgentSnapshot";
import { AgentInspector } from "./AgentInspector";

const agent: AgentSnapshot = {
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
    openness: 0.1,
    conscientiousness: 0.2,
    extraversion: 0.3,
    agreeableness: 0.4,
    neuroticism: 0.5,
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
};

describe("AgentInspector", () => {
  it("shows an empty state before a gecko is selected", () => {
    render(<AgentInspector agent={null} />);

    expect(screen.getByLabelText("Agent inspector")).toBeInTheDocument();
    expect(screen.getByText("Select a gecko")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("shows selected gecko identity, action, needs, mood, and personality", () => {
    render(<AgentInspector agent={agent} />);

    expect(screen.getByRole("heading", { name: "Ada" })).toBeInTheDocument();
    expect(screen.getByText("Walking to Eat snack")).toBeInTheDocument();
    expect(screen.getByText("Target: Fridge")).toBeInTheDocument();
    expect(screen.getByText("Progress: 25%")).toBeInTheDocument();
    expect(screen.getByText("Leaf 1 · (10.0, 12.0)")).toBeInTheDocument();
    expect(screen.getByText("Hunger")).toBeInTheDocument();
    expect(screen.getByText("0.40")).toBeInTheDocument();
    expect(screen.getByText("Valence")).toBeInTheDocument();
    expect(screen.getByText("0.10")).toBeInTheDocument();
    expect(screen.getByText("Openness")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});
