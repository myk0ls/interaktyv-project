import * as THREE from "three";

export class InputHandler {
  constructor(camera, domElement, network) {
    this.camera = camera;
    this.domElement = domElement;
    this.network = network;

    this.mouse = new THREE.Vector2();
    this.raycaster = new THREE.Raycaster();
    this.hitPoint = new THREE.Vector3();

    // shared flat ground plane (Y = 0)
    this.floorPlane = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0);

    this.lookDir = new THREE.Vector3();
    this.currentYaw = 0;

    domElement.addEventListener("pointermove", (e) => this.onPointerMove(e));
    domElement.addEventListener("click", () => this.onShoot());
  }

  onPointerMove(e) {
    const rect = this.domElement.getBoundingClientRect();

    this.mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    this.mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;

    this.updateAim();
  }

  updateAim() {
    this.raycaster.setFromCamera(this.mouse, this.camera);

    const hit = this.raycaster.ray.intersectPlane(
      this.floorPlane,
      this.hitPoint,
    );
    if (!hit) return;

    // IMPORTANT:
    // We do NOT know player position here.
    // Server will rotate relative to authoritative player pos.
    // So we only send yaw.

    const dir = this.hitPoint.clone();
    dir.y = 0;

    if (dir.lengthSq() < 1e-6) return;
    dir.normalize();

    const yaw = Math.atan2(dir.x, dir.z);
    this.currentYaw = yaw;

    this.network.send({
      type: "aim",
      yaw,
    });
  }

  onShoot() {
    console.log("Shooting!");
    this.network.send({
      type: "shoot",
    });
  }
}
