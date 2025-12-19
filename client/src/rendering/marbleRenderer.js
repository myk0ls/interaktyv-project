import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/Addons.js";

/*
MarbleRenderer
- Keeps marbles by id, updates positions and removes when missing
*/
export class MarbleRenderer {
  constructor(scene) {
    this.scene = scene;
    this.marbles = new Map(); // id -> { mesh }
    this.geometry = new THREE.SphereGeometry(0.2, 8, 8);
    this.material = new THREE.MeshStandardMaterial({ color: 0xffcc00 });

    this.red = new THREE.MeshStandardMaterial({ color: 0xff0000 });
    this.blue = new THREE.MeshStandardMaterial({ color: 0x0000ff });
    this.green = new THREE.MeshStandardMaterial({ color: 0x00ff00 });
    this.yellow = new THREE.MeshStandardMaterial({ color: 0xffff00 });
    this.purple = new THREE.MeshStandardMaterial({ color: 0xff00ff });
  }

  update(marblesArray) {
    if (!Array.isArray(marblesArray)) marblesArray = [];

    const seen = new Set();

    for (const m of marblesArray) {
      if (!m || typeof m.id === "undefined") continue;
      seen.add(m.id);

      let entry = this.marbles.get(m.id);
      if (!entry) {
        var entryColor;

        switch (m.color) {
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

        const mesh = new THREE.Mesh(this.geometry, entryColor);
        mesh.castShadow = true;
        mesh.receiveShadow = false;
        this.scene.add(mesh);
        entry = { mesh };
        this.marbles.set(m.id, entry);
      }

      const x = typeof m.x === "number" ? m.x : 0;
      const y = typeof m.y === "number" ? m.y : 0.2;
      const z = typeof m.z === "number" ? m.z : 0;

      entry.mesh.position.set(x, y, z);
    }

    // Remove marbles that no longer exist on server
    for (const [id, entry] of this.marbles.entries()) {
      if (!seen.has(id)) {
        this.scene.remove(entry.mesh);
        if (entry.mesh.geometry) entry.mesh.geometry.dispose();
        if (entry.mesh.material) entry.mesh.material.dispose();
        this.marbles.delete(id);
      }
    }
  }
}
