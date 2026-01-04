import * as THREE from "three";
import { PlayerRenderer } from "./playerRenderer.js";
import { MarbleRenderer } from "./marbleRenderer.js";
import { LevelManager } from "./levelManager.js";
import { UIHandler } from "./uiHandler.js";

/*
  SceneManager
  - container: DOM element to append renderer.domElement into (defaults to body)
  - networkClient: optional WebSocketClient-like instance (has .on and .send)
*/
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
    this.camera = new THREE.PerspectiveCamera(
      75,
      w / Math.max(1, h),
      0.1,
      1000,
    );
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

    this.testCurve();

    // renderers for game objects
    this.playerRenderer = new PlayerRenderer(this.scene);
    this.marbleRenderer = new MarbleRenderer(this.scene);
    this.levelManager = new LevelManager(this.scene);

    this.levelManager.loadLevel(0, "/assets/mainLevel.glb");

    // create UI AFTER the renderer is on the page.
    // pass networkClient into onSendChat so we don't reference an undefined global.
    this.ui = new UIHandler({
      parent: document.body,
      randomScoreIntervalMs: 5000,
      // send via the provided network client if present
      onSendChat: (text) => {
        if (this.network && typeof this.network.send === "function") {
          this.network.send({ type: "chat", text });
        } else {
          console.warn("No network client available to send chat");
        }
      },
    });

    // after creating UIHandler instance:
    document.querySelector(".ui-overlay")?.classList.add("ui-scale-2x");

    // bind network to UI only if available
    if (this.network && typeof this.network.on === "function") {
      this.ui.bindWebSocketClient(this.network);

      // server chat -> UI
      this.network.on("message", (data) => {
        if (data && data.type === "chat") {
          this.ui.addChatMessage(data.author || "Peer", data.text || "");
        }
      });

      // update demo score from server state messages
      this.network.on("state", (state) => {
        const demoScore = Math.floor(Math.random() * 301);
        this.ui.setScore(demoScore);
      });
    } else {
      // no network: show welcome message locally
      this.ui.addChatMessage(
        "System",
        "No network client connected (local mode).",
      );
    }

    // window resize
    window.addEventListener("resize", (e) => this.onWindowResize(e), false);

    // simple clock and start animate loop
    this._lastTime = performance.now();
    this._rafId = null;
    this.start();
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
    this.width = window.innerWidth;
    this.height = window.innerHeight;
    this.camera.aspect = this.width / Math.max(1, this.height);
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(this.width, this.height);
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

  async testCurve() {
    const data = await fetch("/paths/zuma_path.json").then((r) => r.json());
    const points = data.points.map((p) => new THREE.Vector3(...p));

    console.log(points);

    const line = new THREE.Line(
      new THREE.BufferGeometry().setFromPoints(points),
      new THREE.LineBasicMaterial({ color: 0xff0000 }),
    );

    this.scene.add(line);
  }
}
