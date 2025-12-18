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
  }

  update(marblesArray) {
    if (!Array.isArray(marblesArray)) marblesArray = [];

    const seen = new Set();

    for (const m of marblesArray) {
      if (!m || typeof m.id === "undefined") continue;
      seen.add(m.id);

      let entry = this.marbles.get(m.id);
      if (!entry) {
        const mesh = new THREE.Mesh(this.geometry, this.material.clone());
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
