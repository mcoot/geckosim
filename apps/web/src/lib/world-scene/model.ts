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
  if (!world || !snapshot) return EMPTY_WORLD_SCENE_MODEL;

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
