import * as THREE from "three";
import { WebSocketClient } from "./web_socket.js";

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

const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(
  75,
  window.innerWidth / window.innerHeight,
  0.1,
  1000,
);

const renderer = new THREE.WebGLRenderer();
renderer.setSize(window.innerWidth, window.innerHeight);
document.body.appendChild(renderer.domElement);

const geometry = new THREE.BoxGeometry(1, 1, 1);
const material = new THREE.MeshBasicMaterial({ color: 0x00ff00 });
const cube = new THREE.Mesh(geometry, material);
scene.add(cube);

const light = new THREE.PointLight(0xffffff);
light.position.set(-10, 15, 50);
scene.add(light);

camera.position.z = 5;

//controls
const keys = { w: false, a: false, s: false, d: false };

document.addEventListener("keydown", (event) => {
  if (keys[event.key] === false) {
    // Only send once when pressed
    keys[event.key] = true;
    wsClient.send({ type: "key_down", key: event.key });
  }
});

document.addEventListener("keyup", (event) => {
  keys[event.key] = false;
  wsClient.send({ type: "key_up", key: event.key });
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

function animate() {
  // cube.rotation.x += 0.01;
  // cube.rotation.y += 0.01;

  if (pressedKeys.has("a")) {
    cube.rotation.y -= 0.01;
  }
  if (pressedKeys.has("d")) {
    cube.rotation.y += 0.01;
  }
  if (pressedKeys.has("w")) {
    cube.rotation.x -= 0.01;
  }
  if (pressedKeys.has("s")) {
    cube.rotation.x += 0.01;
  }

  renderer.render(scene, camera);
}
renderer.setAnimationLoop(animate);
