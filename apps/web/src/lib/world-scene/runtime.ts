import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { WorldSceneModel } from "./model";
import { populateWorldGroup } from "./scene";

export interface WorldSceneRuntime {
  update: (model: WorldSceneModel) => void;
  dispose: () => void;
}

export function canCreateWorldSceneRuntime(): boolean {
  if (typeof document === "undefined" || typeof window === "undefined") {
    return false;
  }

  try {
    const canvas = document.createElement("canvas");
    return Boolean(
      window.WebGLRenderingContext &&
        (canvas.getContext("webgl2") ||
          canvas.getContext("webgl") ||
          canvas.getContext("experimental-webgl")),
    );
  } catch {
    return false;
  }
}

export function createWorldSceneRuntime(
  mount: HTMLElement,
  initialModel: WorldSceneModel,
): WorldSceneRuntime {
  const renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  mount.appendChild(renderer.domElement);

  const scene = new THREE.Scene();
  scene.background = new THREE.Color("#0a0a0a");

  const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 1000);
  camera.position.set(80, 140, 140);
  camera.lookAt(60, 0, 60);

  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.target.set(60, 0, 60);

  scene.add(new THREE.HemisphereLight("#ffffff", "#334155", 2));

  const root = new THREE.Group();
  root.name = "world-root";
  scene.add(root);

  const resize = () => {
    const width = mount.clientWidth || 640;
    const height = mount.clientHeight || 360;
    renderer.setSize(width, height);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
  };

  const observer = new ResizeObserver(resize);
  observer.observe(mount);
  resize();

  let frame = 0;
  const render = () => {
    frame = window.requestAnimationFrame(render);
    controls.update();
    renderer.render(scene, camera);
  };
  render();

  const update = (model: WorldSceneModel) => {
    populateWorldGroup(root, model);
  };
  update(initialModel);

  return {
    update,
    dispose: () => {
      observer.disconnect();
      window.cancelAnimationFrame(frame);
      controls.dispose();
      renderer.dispose();
      renderer.domElement.remove();
    },
  };
}
