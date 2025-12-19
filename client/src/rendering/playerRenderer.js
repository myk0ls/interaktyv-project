import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";

/*
PlayerRenderer
- Keeps a Map of playerId -> mesh
- update(playersArray) will create new meshes for new players,
  update positions for existing, and remove meshes for players that left.
*/
export class PlayerRenderer {
  constructor(scene) {
    this.scene = scene;
    this.players = new Map(); // id -> { mesh }

    this.previewMarble = new THREE.SphereGeometry(0.2, 8, 8);
    this.playerModel = null;
    this.loader = new GLTFLoader();

    this.loader.load("/assets/mainfrog.glb", (gltf) => {
      this.playerModel = gltf.scene;
      this.playerModel.traverse((obj) => {
        if (obj.isMesh) {
          obj.castShadow = true;
          obj.receiveShadow = false;
        }
      });
    });

    // shared geometry/material for players
    this.geometry = new THREE.CapsuleGeometry(0.4, 0.8, 4, 8);
    this.material = new THREE.MeshStandardMaterial({ color: 0x0077ff });
    this.red = new THREE.MeshStandardMaterial({ color: 0xff0000 });
    this.blue = new THREE.MeshStandardMaterial({ color: 0x0000ff });
    this.green = new THREE.MeshStandardMaterial({ color: 0x00ff00 });
    this.yellow = new THREE.MeshStandardMaterial({ color: 0xffff00 });
    this.purple = new THREE.MeshStandardMaterial({ color: 0xff00ff });
  }

  update(playersArray) {
    if (!Array.isArray(playersArray)) playersArray = [];

    const seen = new Set();

    for (const p of playersArray) {
      // defensive checks
      if (!p || typeof p.id === "undefined") continue;
      seen.add(p.id);

      let entry = this.players.get(p.id);
      if (!entry) {
        const model = this.playerModel.clone(true);
        model.position.set(0, 0, 0);

        this.scene.add(model);
        entry = { mesh: model };
        this.players.set(p.id, entry);
      }

      // Update safely — default zeros if fields missing
      const x = typeof p.x === "number" ? p.x : 0;
      const y = typeof p.y === "number" ? p.y : 0;
      const z = typeof p.z === "number" ? p.z : 0;
      const yaw = typeof p.yaw === "number" ? p.yaw : 0;

      entry.mesh.position.set(x, y, z);
      // orient the capsule: rotate around Y (Three uses rotation.y)
      entry.mesh.rotation.y = yaw;
      //this._setPreviewColor(entry, entry.preview);
    }

    // Remove players that are gone
    for (const [id, entry] of this.players.entries()) {
      if (!seen.has(id)) {
        this.scene.remove(entry.mesh);
        if (entry.mesh.geometry) entry.mesh.geometry.dispose();
        if (entry.mesh.material) entry.mesh.material.dispose();
        this.players.delete(id);
      }
    }
  }

  _setPreviewColor(entry, previewMarble) {
    switch (entry.loader_color) {
      case "red":
        entryColor = this.red;
        break;
      case "blue":
        entryColor = this.blue;
        break;
      case "green":
        entryColor = this.green;
        break;
      case "yellow":
        entryColor = this.yellow;
        break;
      case "purple":
        entryColor = this.purple;
        break;
      default:
        entryColor = this.material;
    }

    previewMarble.material = entryColor;
  }
}
