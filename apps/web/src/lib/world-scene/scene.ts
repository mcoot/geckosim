import * as THREE from "three";
import type {
  AgentRenderModel,
  LeafRenderModel,
  ObjectRenderModel,
  WorldSceneModel,
} from "./model";

export interface PopulateWorldGroupOptions {
  selectedAgentId: number | null;
}

function material(color: string): THREE.MeshStandardMaterial {
  return new THREE.MeshStandardMaterial({ color, metalness: 0, roughness: 0.85 });
}

function lineMaterial(selected: boolean): THREE.LineBasicMaterial {
  return new THREE.LineBasicMaterial({
    color: selected ? "#93c5fd" : "#60a5fa",
    transparent: true,
    opacity: selected ? 0.95 : 0.35,
  });
}

function disposeObject(object: THREE.Object3D): void {
  object.traverse((child) => {
    if (child instanceof THREE.Mesh || child instanceof THREE.Line) {
      child.geometry.dispose();
      const materials = Array.isArray(child.material) ? child.material : [child.material];
      for (const mat of materials) {
        mat.dispose();
      }
    }
  });
}

function addLeaf(root: THREE.Group, leaf: LeafRenderModel): void {
  const mesh = new THREE.Mesh(
    new THREE.PlaneGeometry(leaf.size.x, leaf.size.z),
    material(leaf.color),
  );
  mesh.name = `leaf:${leaf.id}`;
  mesh.position.set(leaf.center.x, -0.02, leaf.center.z);
  mesh.rotation.x = -Math.PI / 2;
  mesh.userData = { leafId: leaf.id, kind: leaf.kind, name: leaf.name };
  root.add(mesh);
}

function addObject(root: THREE.Group, object: ObjectRenderModel): void {
  const mesh = new THREE.Mesh(
    new THREE.BoxGeometry(1.2, 1.2, 1.2),
    material(object.color),
  );
  mesh.name = `object:${object.id}`;
  mesh.position.set(object.position.x, 0.7, object.position.z);
  mesh.userData = {
    objectId: object.id,
    typeId: object.typeId,
    leafId: object.leafId,
  };
  root.add(mesh);
}

function addAgent(root: THREE.Group, agent: AgentRenderModel): void {
  const group = new THREE.Group();
  group.name = `agent:${agent.id}`;
  group.position.set(agent.position.x, 1, agent.position.z);
  group.rotation.y = agent.heading;
  group.userData = {
    agentId: agent.id,
    leafId: agent.leafId,
    phase: agent.phase,
    name: agent.name,
  };

  const body = new THREE.Mesh(
    new THREE.ConeGeometry(1.4, 2.4, 24),
    material(agent.color),
  );
  body.rotation.x = Math.PI / 2;
  body.userData = group.userData;
  group.add(body);
  root.add(group);
}

function addIntentRoute(
  root: THREE.Group,
  agent: AgentRenderModel,
  selected: boolean,
): void {
  const target = agent.intent?.targetPosition;
  if (!target || agent.intent.phase !== "Walking") return;

  const geometry = new THREE.BufferGeometry().setFromPoints([
    new THREE.Vector3(agent.position.x, 0.08, agent.position.z),
    new THREE.Vector3(target.x, 0.08, target.z),
  ]);
  const line = new THREE.Line(geometry, lineMaterial(selected));
  line.name = `intent-route:${agent.id}`;
  line.userData = {
    agentId: agent.id,
    targetObjectId: agent.intent.targetObjectId,
    kind: "intent-route",
    selected,
  };
  root.add(line);
}

function addSelectedTarget(root: THREE.Group, agent: AgentRenderModel): void {
  const target = agent.intent?.targetPosition;
  if (!target) return;

  const marker = new THREE.Mesh(
    new THREE.RingGeometry(1.4, 1.9, 32),
    new THREE.MeshBasicMaterial({
      color: "#fbbf24",
      transparent: true,
      opacity: 0.9,
      side: THREE.DoubleSide,
    }),
  );
  marker.name = `intent-target:${agent.id}`;
  marker.position.set(target.x, 0.1, target.z);
  marker.rotation.x = -Math.PI / 2;
  marker.userData = {
    agentId: agent.id,
    targetObjectId: agent.intent.targetObjectId,
    kind: "intent-target",
  };
  root.add(marker);
}

export function populateWorldGroup(
  root: THREE.Group,
  model: WorldSceneModel,
  options: PopulateWorldGroupOptions = { selectedAgentId: null },
): void {
  for (const child of [...root.children]) {
    root.remove(child);
    disposeObject(child);
  }

  for (const leaf of model.leaves) addLeaf(root, leaf);
  for (const object of model.objects) addObject(root, object);
  for (const agent of model.agents) {
    addIntentRoute(root, agent, agent.id === options.selectedAgentId);
  }
  for (const agent of model.agents) {
    if (agent.id === options.selectedAgentId) addSelectedTarget(root, agent);
  }
  for (const agent of model.agents) addAgent(root, agent);
}
