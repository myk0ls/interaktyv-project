import * as THREE from "three";
import { WebSocketClient } from "./core/webSocketClient.js";
import { GLTFLoader } from "three/examples/jsm/Addons.js";

const wsClient = new WebSocketClient("ws://127.0.0.1:8080");

wsClient.on("open", () => {
  console.log("Connected!");

  // Send a test message
  wsClient.send({
    type: "hello",
    message: "Hello from client!",
  });
});

// Handle incoming messages
wsClient.on("message", (data) => {
  console.log("Received:", data);
});

// setInterval(() => {
//   wsClient.send({
//     type: "test",
//     timestamp: Date.now(),
//     message: "Ping from client",
//   });
// }, 3000);

//scene
const scene = new THREE.Scene();
scene.background = new THREE.Color(0xa8def0);

//camera
const camera = new THREE.PerspectiveCamera(
  75,
  window.innerWidth / window.innerHeight,
  0.1,
  1000,
);

camera.position.y = 5;
camera.position.z = 1;
camera.rotateX(-1.4);

//renderer
const renderer = new THREE.WebGLRenderer();
renderer.setSize(window.innerWidth, window.innerHeight);
renderer.setPixelRatio(window.devicePixelRatio);
renderer.shadowMap.enabled = true;
document.body.appendChild(renderer.domElement);

//controls
// const orbitControls = new OrbitControls(camera, renderer.domElement);
// orbitControls.enableDamping = true;
// orbitControls.minDistance = 2;
// orbitControls.maxDistance = 15;
// orbitControls.enablePan = false;
// orbitControls.maxPolarAngle = Math.PI / 2 - 0.05;
// //orbitControls.enableRotate = false;
// orbitControls.update();

//cube
const geometry = new THREE.BoxGeometry(1, 1, 1);
const material = new THREE.MeshBasicMaterial({ color: 0x00ff00 });
const cube = new THREE.Mesh(geometry, material);
material.visible = false;
scene.add(cube);

new GLTFLoader().load("/assets/mainfrog.glb", function (gltf) {
  const frog = gltf.scene;
  cube.traverse(function (object) {
    if (object.isMesh) object.castShadow = true;
  });
  cube.add(frog);
});

const grid = new THREE.GridHelper(40, 40, (1, 1, 1), (1, 1, 1));
scene.add(grid);
grid.position.y = 0.1;

generateFloor();

//detectionfloor
const geo = new THREE.PlaneGeometry(80, 80, 512, 512);
//const material = new THREE.MeshStandardMaterial({ color: 0x00ff00,  });
const mat = material.clone();
mat.visible = false;

//const floor = new THREE.Plane();
const floor = new THREE.Plane(new THREE.Vector3(0, 1, 0), -cube.position.y);
//floor.rotation.x = -Math.PI / 2;
//scene.add(floor);

// const light = new THREE.PointLight(0xffffff);
// light.position.set(-10, 15, 50);
// scene.add(light);

light();

//modelll
// var characterControls;
// new GLTFLoader().load("./src/assets/Soldier.glb", function (gltf) {
//   const model = gltf.scene;
//   model.traverse(function (object) {
//     if (object.isMesh) object.castShadow = true;
//   });
//   scene.add(model);

//   const gltfAnimations = gltf.animations;
//   const mixer = new THREE.AnimationMixer(model);
//   const animationMap = new Map();
//   gltfAnimations
//     .filter((a) => a.name != "TPose")
//     .forEach((a) => {
//       animationMap.set(a.name, mixer.clipAction(a));
//     });

//   characterControls = new CharacterControls(
//     model,
//     mixer,
//     animationMap,
//     orbitControls,
//     camera,
//     "Idle",
//   );
// });

//controls

const marbles = [];

const raycaster = new THREE.Raycaster();
const mouse = new THREE.Vector2();
const hitPoint = new THREE.Vector3();
var lookDir = new THREE.Vector3();

window.addEventListener("resize", () => {
  camera.aspect = innerWidth / innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(innerWidth, innerHeight);
});

window.addEventListener("pointermove", (e) => {
  mouse.x = (e.clientX / innerWidth) * 2 - 1;
  mouse.y = -(e.clientY / innerHeight) * 2 + 1;
});

const keys = { w: false, a: false, s: false, d: false };

document.addEventListener("keydown", (event) => {
  if (keys[event.key] === false) {
    // Only send once when pressed
    keys[event.key] = true;
    wsClient.send({ type: "key_down", key: event.key.toLowerCase() });
  }
});

document.addEventListener("keyup", (event) => {
  keys[event.key] = false;
  wsClient.send({ type: "key_up", key: event.key.toLowerCase() });
});

const pressedKeys = new Map();

wsClient.on("message", (data) => {
  if (data.type == "key_down") {
    pressedKeys.set(data.key, true);
  }
  if (data.type == "key_up") {
    pressedKeys.delete(data.key);
  }
});

// Assume you already have your renderer and canvas set up:
const canvas = renderer.domElement;

// // Ask for pointer lock when clicking the canvas
// canvas.addEventListener("click", () => {
//   canvas.requestPointerLock();
// });

// // Listen for changes in pointer lock
// document.addEventListener("pointerlockchange", () => {
//   if (document.pointerLockElement === canvas) {
//     console.log("Pointer locked!");
//     // Start listening for mouse movement
//     document.addEventListener("mousemove", onMouseMove);
//   } else {
//     console.log("Pointer unlocked!");
//     // Stop listening for mouse movement
//     document.removeEventListener("mousemove", onMouseMove);
//   }
// });

// Option: flip sign if your model faces the opposite way
const INVERT_YAW = false; // set to true if it looks mirrored

function updateTurretYaw() {
  raycaster.setFromCamera(mouse, camera);

  const intersect = raycaster.ray.intersectPlane(floor, hitPoint);
  if (!intersect) return;

  const dir = hitPoint.clone().sub(cube.position);
  dir.y = 0;
  if (dir.lengthSq() < 1e-6) return;

  dir.normalize();

  lookDir = dir;

  // yaw so local +Z maps to dir: (sinθ, cosθ) => θ = atan2(dx, dz)
  const angle = Math.atan2(dir.x, dir.z);
  cube.rotation.y = INVERT_YAW ? -angle : angle;
}

canvas.addEventListener("click", () => {
  const geometry = new THREE.SphereGeometry(0.25);
  const material = new THREE.MeshBasicMaterial({ color: 0x00ff00 });
  const marble = new THREE.Mesh(geometry, material);

  //marble.position.copy(cube.position).add(lookDir.clone().multiplyScalar(1));
  marble.position.copy(cube.position);

  const speed = 0.05;
  marble.userData.velocity = lookDir.clone().multiplyScalar(speed);

  scene.add(marble);
  marbles.push(marble);
  console.log("ATSPAWNIWNO CAMUOLI");
});

const clock = new THREE.Clock();
function animate() {
  // cube.rotation.x += 0.01;
  // cube.rotation.y += 0.01;

  // let mixerUpdateDelta = clock.getDelta();
  // if (characterControls) {
  //   characterControls.update(mixerUpdateDelta, pressedKeys);
  // }

  // if (pressedKeys.has("a")) {
  //   cube.rotation.y -= 0.01;
  // }
  // if (pressedKeys.has("d")) {
  //   cube.rotation.y += 0.01;
  // }
  // if (pressedKeys.has("w")) {
  //   cube.rotation.x -= 0.01;
  // }
  // if (pressedKeys.has("s")) {
  //   cube.rotation.x += 0.01;
  // }

  //orbitControls.update();

  updateTurretYaw();

  for (let i = marbles.length - 1; i >= 0; i--) {
    const m = marbles[i];
    m.position.add(m.userData.velocity);

    // optional: remove if too far away
    if (m.position.length() > 200) {
      scene.remove(m);
      marbles.splice(i, 1);
    }
  }

  renderer.render(scene, camera);
}
renderer.setAnimationLoop(animate);

function generateFloor() {
  // TEXTURES
  const textureLoader = new THREE.TextureLoader();
  const grass = textureLoader.load("./src/assets/grass.jpg");

  const WIDTH = 80;
  const LENGTH = 80;

  const geometry = new THREE.PlaneGeometry(WIDTH, LENGTH, 512, 512);
  //const material = new THREE.MeshStandardMaterial({ color: 0x00ff00,  });
  const material = new THREE.MeshStandardMaterial({ map: grass });
  wrapAndRepeatTexture(material.map);
  // const material = new THREE.MeshPhongMaterial({ map: placeholder})

  const floor = new THREE.Mesh(geometry, material);
  floor.receiveShadow = true;
  floor.rotation.x = -Math.PI / 2;
  scene.add(floor);
}

function wrapAndRepeatTexture(map) {
  map.wrapS = map.wrapT = THREE.RepeatWrapping;
  map.repeat.x = map.repeat.y = 10;
}

function light() {
  scene.add(new THREE.AmbientLight(0xffffff, 0.7));

  const dirLight = new THREE.DirectionalLight(0xffffff, 1);
  dirLight.position.set(-60, 100, -10);
  dirLight.castShadow = true;
  dirLight.shadow.camera.top = 50;
  dirLight.shadow.camera.bottom = -50;
  dirLight.shadow.camera.left = -50;
  dirLight.shadow.camera.right = 50;
  dirLight.shadow.camera.near = 0.1;
  dirLight.shadow.camera.far = 200;
  dirLight.shadow.mapSize.width = 4096;
  dirLight.shadow.mapSize.height = 4096;
  scene.add(dirLight);
  // scene.add( new THREE.CameraHelper(dirLight.shadow.camera))
}

// function onMouseMove(event) {
//   // event.movementX and event.movementY give mouse deltas
//   if (document.pointerLockElement === renderer.domElement) {
//     // sensitivity
//     const sensitivity = 0.002;
//     orbitControls.rotateLeft(event.movementX * sensitivity);
//     orbitControls.rotateUp(event.movementY * sensitivity);
//   }
// }

//function onMouseMove(e) {}
