import SceneManager from "../rendering/sceneManager.js";
import { WebSocketClient } from "./webSocketClient.js";
import { InputHandler } from "../input/inputHandler.js";
import Stats from "stats.js";

export default class GameClient {
  constructor() {
    this.sceneManager = new SceneManager();
    this.network = new WebSocketClient("ws://127.0.0.1:8080");

    this.input = new InputHandler(
      this.sceneManager.camera,
      this.sceneManager.renderer.domElement,
      this.network,
    );

    this.gameState = { players: [], marbles: [] };

    this.network.on("state", (s) => (this.gameState = s));

    this.lastTime = performance.now();
    this.running = false;
  }

  loop() {
    requestAnimationFrame(() => this.loop());

    const now = performance.now();
    const dt = (now - this.lastTime) / 1000;
    this.lastTime = now;

    this.sceneManager.update(dt, this.gameState);
    this.sceneManager.render();
  }
}
