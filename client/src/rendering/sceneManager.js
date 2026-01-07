import * as THREE from "three";
import { PlayerRenderer } from "./playerRenderer.js";
import { MarbleRenderer } from "./marbleRenderer.js";
import { LevelManager } from "./levelManager.js";

export default class SceneManager {
  constructor({ container = document.body, networkClient = null } = {}) {
    this.container = container;
    this.network = networkClient; // may be null

    // scene
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0xa8def0);

    // camera
    const w = window.innerWidth;
    const h = window.innerHeight;
    this.camera = new THREE.OrthographicCamera(
      w / -248,
      w / 248,
      h / 248,
      h / -248,
    );
    // this.camera = new THREE.PerspectiveCamera(
    //   75,
    //   w / Math.max(1, h),
    //   0.1,
    //   1000,
    // );
    this.camera.position.y = 5.5;
    this.camera.position.z = 0;
    this.camera.rotateX(-1.6);

    // renderer (fixed order)
    this.renderer = new THREE.WebGLRenderer({ antialias: true });

    // clamp DPR to avoid huge backing buffers on high-DPI displays
    const DPR = Math.min(window.devicePixelRatio || 1, 2);
    this.renderer.setPixelRatio(DPR);

    // now set the canvas size (uses the DPR to set backing-store size)
    this.renderer.setSize(w, h);

    this.renderer.shadowMap.enabled = true;

    // append canvas BEFORE UI so overlay z-index is above canvas
    this.container.appendChild(this.renderer.domElement);
    this._ensureCanvasVisible(this.renderer.domElement);

    // lights & floor
    this.initLights();
    //this.initFloor();

    //this.testCurve("./paths/zuma_path.json");

    // renderers for game objects
    this.playerRenderer = new PlayerRenderer(this.scene);
    this.marbleRenderer = new MarbleRenderer(this.scene);
    this.levelManager = new LevelManager(this.scene);

    // Track which GLB is currently loaded so we can switch levels on join
    this.currentLevelId = null;
    this.currentLevelKey = null;

    // Default to second-level for backwards compatibility
    this.setLevelByKey("second-level");

    // window resize
    window.addEventListener("resize", (e) => this.onWindowResize(e), false);

    // simple clock and start animate loop
    this._lastTime = performance.now();
    this._rafId = null;
    this.start();
  }

  // Map server/client "level" keys to GLB files
  _levelKeyToGlbPath(levelKey) {
    switch (levelKey) {
      case "first-level":
        return "./assets/firstLevel.glb";
      case "second-level":
        return "./assets/secondLevel.glb";
      default:
        // safe fallback
        return "./assets/secondLevel.glb";
    }
  }

  // Public: switch the loaded level (GLB) at runtime
  setLevelByKey(levelKey) {
    if (!levelKey) return;

    // no-op if already loaded
    if (this.currentLevelKey === levelKey) return;

    // unload previous if needed
    if (this.currentLevelId != null) {
      this.levelManager.unloadLevel(this.currentLevelId);
      this.currentLevelId = null;
    }

    const glbPath = this._levelKeyToGlbPath(levelKey);
    const levelId = levelKey;

    this.levelManager.loadLevel(levelId, glbPath);
    this.currentLevelId = levelId;
    this.currentLevelKey = levelKey;
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

    const greyMaterial = new THREE.MeshStandardMaterial({ color: 0x808080 });

    const floor = new THREE.Mesh(
      new THREE.PlaneGeometry(80, 80),
      //new THREE.MeshStandardMaterial({ map: tex }),
      greyMaterial,
    );

    floor.rotation.x = -Math.PI / 2;
    floor.receiveShadow = true;
    this.scene.add(floor);
  }

  // public update called with external gameState if you have it (keeps renderers in sync)
  update(dt, gameState) {
    // protect against missing gameState
    if (gameState && gameState.players) {
      this.playerRenderer.update(gameState.players);
    }
    if (gameState && gameState.marbles) {
      this.marbleRenderer.update(gameState.marbles);
    }

    // Interpolate positions every frame for smooth movement
    this.playerRenderer.interpolate(dt);
    this.marbleRenderer.interpolate(dt);
  }

  render() {
    this.renderer.render(this.scene, this.camera);
  }

  // ---------- animation loop ----------
  start() {
    if (this._rafId) return;
    this._lastTime = performance.now();
    const loop = (t) => {
      const dt = (t - this._lastTime) / 1000;
      this._lastTime = t;
      // If you have authoritative game state, you may call update here with it.
      // For now just render and let external code call SceneManager.update when it receives 'state'.
      this.render();
      this._rafId = requestAnimationFrame(loop);
    };
    this._rafId = requestAnimationFrame(loop);
  }

  stop() {
    if (this._rafId) cancelAnimationFrame(this._rafId);
    this._rafId = null;
  }

  onWindowResize() {
    const w = window.innerWidth;
    const h = window.innerHeight;
    const zoom = 248; // Your original scale factor

    this.camera.left = w / -zoom;
    this.camera.right = w / zoom;
    this.camera.top = h / zoom;
    this.camera.bottom = h / -zoom;

    this.camera.updateProjectionMatrix();

    this.renderer.setSize(w, h);
  }

  // Utility to ensure canvas is full-screen and behind UI
  _ensureCanvasVisible(rendererDomElement) {
    if (!rendererDomElement) return;
    rendererDomElement.style.display = "block";
    rendererDomElement.style.position = "fixed";
    rendererDomElement.style.inset = "0";
    rendererDomElement.style.width = "100%";
    rendererDomElement.style.height = "100%";
    rendererDomElement.style.zIndex = "0";
    // ensure body has no margin to avoid scrollbars
    document.body.style.margin = "0";
  }

  async testCurve(filepath) {
    const data = await fetch(filepath).then((r) => r.json());
    const points = data.points.map((p) => new THREE.Vector3(...p));

    console.log(points);

    const line = new THREE.Line(
      new THREE.BufferGeometry().setFromPoints(points),
      new THREE.LineBasicMaterial({ color: 0xff0000 }),
    );

    this.scene.add(line);
  }
}
