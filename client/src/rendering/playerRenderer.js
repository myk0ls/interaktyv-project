import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";

/*
PlayerRenderer
- Keeps a Map of playerId -> entry { mesh, preview }
- update(playersArray) will create new meshes for new players,
  update positions for existing, and remove meshes for players that left.
*/
export class PlayerRenderer {
  constructor(scene) {
    this.scene = scene;
    this.players = new Map(); // id -> { mesh, preview }

    // preview marble geometry (small sphere)
    this.previewGeometry = new THREE.SphereGeometry(0.2, 8, 8);
    this.playerModel = null;
    this.loader = new GLTFLoader();

    // shared geometry/material for fallback/placeholder model
    this.placeholderGeometry = new THREE.CapsuleGeometry(0.4, 0.8, 4, 8);
    this.placeholderMaterial = new THREE.MeshStandardMaterial({
      color: 0x0077ff,
    });

    // materials for preview marble colors (shared instances)
    this.materials = {
      red: new THREE.MeshStandardMaterial({ color: 0xff0000 }),
      blue: new THREE.MeshStandardMaterial({ color: 0x0000ff }),
      green: new THREE.MeshStandardMaterial({ color: 0x00ff00 }),
      yellow: new THREE.MeshStandardMaterial({ color: 0xffff00 }),
      purple: new THREE.MeshStandardMaterial({ color: 0xff00ff }),
      default: new THREE.MeshStandardMaterial({ color: 0x888888 }),
    };

    // load the GLTF player model asynchronously
    this.loader.load("/assets/mainfrog.glb", (gltf) => {
      this.playerModel = gltf.scene;
      this.playerModel.traverse((obj) => {
        if (obj.isMesh) {
          obj.castShadow = true;
          obj.receiveShadow = false;
        }
      });

      // Replace placeholders with the loaded model for any already-created players
      for (const entry of this.players.values()) {
        if (entry && entry.mesh && entry.isPlaceholder) {
          const modelClone = this.playerModel.clone(true);
          modelClone.position.copy(entry.mesh.position);
          modelClone.rotation.copy(entry.mesh.rotation);
          // remove placeholder and add real model
          this.scene.remove(entry.mesh);
          // dispose placeholder geometry/material only if unique (we reuse shared ones so don't dispose)
          entry.mesh = modelClone;
          entry.isPlaceholder = false;
          entry.mesh.add(entry.preview); // attach preview to the model
          this.scene.add(entry.mesh);
        }
      }
    });
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
        // create either a cloned model (if loaded) or a placeholder mesh
        let model;
        let isPlaceholder = false;
        if (this.playerModel) {
          model = this.playerModel.clone(true);
        } else {
          model = new THREE.Mesh(
            this.placeholderGeometry,
            this.placeholderMaterial,
          );
          isPlaceholder = true;
        }
        // ensure model is a Group/ Object3D so we can attach preview as child
        const group = new THREE.Group();
        group.add(model);
        group.position.set(0, 0, 0);

        // create preview sphere and attach it to the group (above the head)
        const previewMat = this.materials.default;
        const previewMesh = new THREE.Mesh(this.previewGeometry, previewMat);
        // position preview relative to the model's origin; tweak Y to sit above the frog
        previewMesh.position.set(0, 0.65, 0);
        previewMesh.castShadow = true;
        previewMesh.receiveShadow = false;
        group.add(previewMesh);

        this.scene.add(group);
        entry = {
          mesh: group,
          preview: previewMesh,
          isPlaceholder,
        };
        this.players.set(p.id, entry);
      }

      // Update safely — default zeros if fields missing
      const x = typeof p.x === "number" ? p.x : 0;
      const y = typeof p.y === "number" ? p.y : 0;
      const z = typeof p.z === "number" ? p.z : 0;
      const yaw = typeof p.yaw === "number" ? p.yaw : 0;

      entry.mesh.position.set(x, y, z);
      // orient the model around Y (three uses rotation.y)
      entry.mesh.rotation.y = yaw;

      // update preview color/material using player's loaded_color
      const loadedColor =
        typeof p.loaded_color === "string" ? p.loaded_color : null;
      this._setPreviewColor(entry, loadedColor);
    }

    // Remove players that are gone
    for (const [id, entry] of this.players.entries()) {
      if (!seen.has(id)) {
        // remove preview and model from scene and dispose geometries if appropriate
        if (entry.preview) {
          entry.mesh.remove(entry.preview);
          if (entry.preview.geometry) entry.preview.geometry.dispose();
          // do not dispose preview.material if it's shared; our materials map is shared
        }
        // if we used a placeholder mesh, it shares geometry/material so don't dispose global ones
        this.scene.remove(entry.mesh);
        // If the model clone uses unique geometries/materials inside GLTF, disposing them can be complicated,
        // so we avoid disposing model internals (three.js GLTF clones share geometries by default).
        this.players.delete(id);
      }
    }
  }

  _setPreviewColor(entry, color) {
    // pick material from map
    let mat = this.materials.default;
    if (color && typeof color === "string") {
      switch (color) {
        case "red":
          mat = this.materials.red;
          break;
        case "blue":
          mat = this.materials.blue;
          break;
        case "green":
          mat = this.materials.green;
          break;
        case "yellow":
          mat = this.materials.yellow;
          break;
        case "purple":
          mat = this.materials.purple;
          break;
        default:
          mat = this.materials.default;
      }
    }

    // assign material (shared) to preview mesh
    if (entry.preview) {
      entry.preview.material = mat;
    }
  }
}
