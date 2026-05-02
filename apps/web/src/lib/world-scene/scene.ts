import * as THREE from "three";
import type {
  AgentRenderModel,
  LeafRenderModel,
  ObjectRenderModel,
  WorldSceneModel,
} from "./model";

function material(color: string): THREE.MeshStandardMaterial {
  return new THREE.MeshStandardMaterial({ color, metalness: 0, roughness: 0.85 });
}

function disposeObject(object: THREE.Object3D): void {
  object.traverse((child) => {
    if (child instanceof THREE.Mesh) {
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

export function populateWorldGroup(root: THREE.Group, model: WorldSceneModel): void {
  for (const child of [...root.children]) {
    root.remove(child);
    disposeObject(child);
  }

  for (const leaf of model.leaves) addLeaf(root, leaf);
  for (const object of model.objects) addObject(root, object);
  for (const agent of model.agents) addAgent(root, agent);
}
