import * as THREE from "three";
import { PlayerRenderer } from "./playerRenderer.js";
import { MarbleRenderer } from "./marbleRenderer.js";

export default class SceneManager {
  // scene;
  // width;
  // height;
  // camera;
  // renderer;
  // controls = false;

  constructor(domElement = document.getElementById("gl_context")) {
    //scene
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0xa8def0);

    //heightwidth
    this.width = window.innerWidth;
    this.height = window.innerHeight;

    //camera
    this.camera = new THREE.PerspectiveCamera(
      75,
      window.innerWidth / window.innerHeight,
      0.1,
      1000,
    );

    this.camera.position.y = 5;
    this.camera.position.z = 1;
    this.camera.rotateX(-1.4);

    //renderer
    this.renderer = new THREE.WebGLRenderer();
    this.renderer.setSize(window.innerWidth, window.innerHeight);
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.shadowMap.enabled = true;

    document.body.appendChild(this.renderer.domElement);

    //listeners
    window.addEventListener("resize", (e) => this.onWindowResize(e), false);

    this.initLights();
    this.initFloor();

    this.playerRenderer = new PlayerRenderer(this.scene);
    this.marbleRenderer = new MarbleRenderer(this.scene);
  }

  initLights() {
    this.scene.add(new THREE.AmbientLight(0xffffff, 0.7));

    const sun = new THREE.DirectionalLight(0xffffff, 1);
    sun.position.set(-60, 100, -10);
    sun.castShadow = true;
    this.scene.add(sun);
  }

  initFloor() {
    const tex = new THREE.TextureLoader().load("./src/assets/grass.jpg");
    tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
    tex.repeat.set(10, 10);

    const floor = new THREE.Mesh(
      new THREE.PlaneGeometry(80, 80),
      new THREE.MeshStandardMaterial({ map: tex }),
    );

    floor.rotation.x = -Math.PI / 2;
    floor.receiveShadow = true;
    this.scene.add(floor);
  }

  update(dt, gameState) {
    this.playerRenderer.update(gameState.players);
    this.marbleRenderer.update(gameState.marbles);
  }

  render() {
    this.renderer.render(this.scene, this.camera);
  }

  onWindowResize(e) {
    this.width = window.innerWidth;
    //this.height = Math.floor(window.innerHeight - window.innerHeight * 0.3);
    this.height = window.innerHeight;
    this.camera.aspect = this.width / this.height;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(this.width, this.height);
  }

  onLeaveCanvas(e) {
    this.controls.enabled = false;
  }

  onEnterCanvas(e) {
    this.controls.enabled = true;
  }
}
