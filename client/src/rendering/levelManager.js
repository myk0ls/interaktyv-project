import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";

export class LevelManager {
  constructor(scene) {
    this.scene = scene;
    this.levels = new Map(); // levelId -> { group, loaded }
    this.loader = new GLTFLoader();
  }

  loadLevel(levelId, path) {
    if (this.levels.has(levelId)) {
      console.warn(`Level ${levelId} is already loaded.`);
      return;
    }

    this.loader.load(
      path,
      (gltf) => {
        const levelGroup = new THREE.Group();
        levelGroup.name = `level-${levelId}`;
        levelGroup.add(gltf.scene);
        this.scene.add(levelGroup);
        this.levels.set(levelId, { group: levelGroup, loaded: true });
        console.log(`Level ${levelId} loaded from ${path}.`);
      },
      undefined,
      (error) => {
        console.error(`Error loading level ${levelId} from ${path}:`, error);
      },
    );
  }

  unloadLevel(levelId) {
    const levelEntry = this.levels.get(levelId);
    if (!levelEntry) {
      console.warn(`Level ${levelId} is not loaded.`);
      return;
    }

    this.scene.remove(levelEntry.group);
    // Dispose of geometries, materials, and textures to free memory
    levelEntry.group.traverse((obj) => {
      if (obj.isMesh) {
        obj.geometry.dispose();
        if (Array.isArray(obj.material)) {
          obj.material.forEach((mat) => mat.dispose());
        } else {
          obj.material.dispose();
        }
      }
    });

    this.levels.delete(levelId);
    console.log(`Level ${levelId} unloaded.`);
  }
}
//         preview.position.set(0, 2.5, 0);
//         group.add(preview);
//
//         this.scene.add(group);
//
//         entry = {
//           mesh: group,
//           targetPos: new THREE.Vector3(),
//           currentPos: new THREE.Vector3(),
//           velocity: new THREE.Vector3(),
//           isPlaceholder: isPlaceholder,
//           preview: preview,
//         };
//         this.players.set(p.id, entry);
//       }
//    // update target position and rotation from server
