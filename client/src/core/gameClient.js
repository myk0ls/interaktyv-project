import SceneManager from "../rendering/sceneManager.js";
import { WebSocketClient } from "./webSocketClient.js";
import { InputHandler } from "../input/inputHandler.js";
import MainMenu from "./mainMenu.js";
import Stats from "stats.js";
import { UIHandler } from "../rendering/uiHandler.js";

export default class GameClient {
  constructor() {
    // network client
    //this.network = new WebSocketClient("ws://127.0.0.1:8080");

    this.network = new WebSocketClient(
      "wss://wryly-unridiculed-bret.ngrok-free.dev/ws",
    );

    //this.network = new WebSocketClient("ws://localhost:8080/ws");

    // UI is owned by GameClient to ensure a single overlay instance.
    this.ui = new UIHandler({
      parent: document.body,
    });
    if (this.network && typeof this.network.on === "function") {
      this.ui.bindWebSocketClient(this.network);
    }

    this.sceneManager = new SceneManager({
      container: document.body,
      networkClient: this.network,
    });

    // input handling (camera, canvas, network)
    this.input = new InputHandler(
      this.sceneManager.camera,
      this.sceneManager.renderer.domElement,
      this.network,
    );

    // debug
    // simple stats overlay
    this.stats = new Stats();
    this.stats.showPanel(0); // fps
    document.body.appendChild(this.stats.dom);

    // game state placeholder
    this.gameState = { players: [], marbles: [] };

    // receive authoritative state from server
    this.network.on("state", (s) => {
      this.gameState = s;

      // propagate server state score into UI (server-authoritative)
      const score =
        s && typeof s.score === "number"
          ? s.score
          : s && typeof s.score === "string"
            ? Number(s.score)
            : null;

      if (score != null && Number.isFinite(score)) {
        if (this.ui && typeof this.ui.setScore === "function") {
          this.ui.setScore(score);
        }
      }
    });

    // create the main menu and show it initially
    this.menu = new MainMenu({
      parent: document.body,
      networkClient: this.network,
    });

    // When user clicks Join on a room, we'll send a join request and start the client loop
    this.menu.onJoin((room) => {
      console.log("Requested join room:", room);
      // Use the proper joinRoom method
      if (this.network && typeof this.network.joinRoom === "function") {
        this.network.joinRoom(room.id);
      }
      // hide menu and start the game loop
      this.menu.hide();
      if (!this.running) {
        this.startLoop();
      }
    });

    // show the menu
    this.menu.show();

    // flags for loop
    this.lastTime = performance.now();
    this.running = false;
  }

  // Start the RAF loop
  startLoop() {
    if (this.running) return;
    this.running = true;
    this.lastTime = performance.now();
    const tick = (now) => {
      if (!this.running) return;
      this.stats.begin();

      const dt = (now - this.lastTime) / 1000;
      this.lastTime = now;

      // Update input handler with local player position
      if (this.gameState && this.gameState.players) {
        // Find the local player (you'll need to store the player ID when joining)
        const localPlayer = this.gameState.players.find(
          (p) => p.id === this.network.myPlayerId,
        );
        if (localPlayer) {
          this.input.setPlayerPosition(
            localPlayer.x || 0,
            localPlayer.y || 0,
            localPlayer.z || 0,
          );
        }
      }

      // update renderers with latest authoritative state
      this.sceneManager.update(dt, this.gameState);
      this.sceneManager.render();

      this.stats.end();
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }

  // Stop the RAF loop
  stopLoop() {
    this.running = false;
  }
}
