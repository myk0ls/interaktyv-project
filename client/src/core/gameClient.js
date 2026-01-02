// import SceneManager from "../rendering/sceneManager.js";
// import { WebSocketClient } from "./webSocketClient.js";
// import { InputHandler } from "../input/inputHandler.js";
// import Stats from "stats.js";

// export default class GameClient {
//   constructor() {
//     this.network = new WebSocketClient("ws://127.0.0.1:8080");
//     this.sceneManager = new SceneManager(this.network);

//     this.input = new InputHandler(
//       this.sceneManager.camera,
//       this.sceneManager.renderer.domElement,
//       this.network,
//     );

//     this.gameState = { players: [], marbles: [] };

//     this.network.on("state", (s) => (this.gameState = s));

//     this.lastTime = performance.now();
//     this.running = false;
//   }

//   loop() {
//     requestAnimationFrame(() => this.loop());

//     const now = performance.now();
//     const dt = (now - this.lastTime) / 1000;
//     this.lastTime = now;

//     this.sceneManager.update(dt, this.gameState);
//     this.sceneManager.render();
//   }
// }

import SceneManager from "../rendering/sceneManager.js";
import { WebSocketClient } from "./webSocketClient.js";
import { InputHandler } from "../input/inputHandler.js";
import MainMenu from "./mainMenu.js";
import Stats from "stats.js";

export default class GameClient {
  constructor() {
    // network client
    this.network = new WebSocketClient("ws://127.0.0.1:8080");

    // create scene manager and pass network client so SceneManager can bind UI etc.
    // SceneManager expects an options object in the current codebase.
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

    // simple stats overlay (optional)
    this.stats = new Stats();
    this.stats.showPanel(0); // fps
    document.body.appendChild(this.stats.dom);

    // game state placeholder
    this.gameState = { players: [], marbles: [] };

    // receive authoritative state from server
    this.network.on("state", (s) => {
      this.gameState = s;
    });

    // create the main menu (mock) and show it initially
    this.menu = new MainMenu({
      parent: document.body,
      // you can seed rooms here if you want: rooms: [...]
    });

    // When user clicks Join on a room, we'll send a join request and start the client loop
    this.menu.onJoin((room) => {
      console.log("Requested join room:", room);
      // send a mock join request to server (server should handle it if implemented)
      if (this.network && typeof this.network.send === "function") {
        this.network.send({ type: "join_room", roomId: room.id });
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
