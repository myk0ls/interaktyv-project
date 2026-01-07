export class UIHandler {
  constructor(opts = {}) {
    this.parent = opts.parent || document.body;
    this.onSendChat =
      typeof opts.onSendChat === "function" ? opts.onSendChat : null;
    this.maxMessages = opts.maxMessages || 200;

    this._createStyles();
    this._createElements();

    // seed the score with a random value 0..300 as requested
    this.setScore(this._randomScore());
    // optional periodic random score update for demo/debug:
    if (opts.randomScoreIntervalMs) {
      this._scoreTimer = setInterval(() => {
        this.setScore(this._randomScore());
      }, opts.randomScoreIntervalMs);
    }
  }

  destroy() {
    if (this.container && this.parent.contains(this.container)) {
      this.parent.removeChild(this.container);
    }
    if (this._scoreTimer) {
      clearInterval(this._scoreTimer);
      this._scoreTimer = null;
    }
  }

  // Replace UIHandler._createStyles() with the code below
  _createStyles() {
    if (document.getElementById("game-ui-styles")) return;
    const style = document.createElement("style");
    style.id = "game-ui-styles";
    style.textContent = `
      :root {
        --ui-font-size: 18px;
        --ui-small-size: 13px;
        --ui-radius: 12px;
        --ui-gap: 14px;
        --ui-chat-width: 420px;
        --ui-z: 9999;
      }

      /* Make sure the overlay is not affected by parent transforms or scaling */
      .ui-overlay {
        position: fixed;
        inset: 0;
        pointer-events: none;
        z-index: var(--ui-z);
        font-family: system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial;

        /* Replace 'transparent' with one of the options below */
        background-color: transparent; /* Semi-transparent Grey */

        -webkit-font-smoothing: antialiased;
        -moz-osx-font-smoothing: grayscale;
        transform-origin: top left;
      }

      /* Score box (top-left) */
      .ui-score {
        position: absolute;
        top: var(--ui-gap);
        left: var(--ui-gap);
        pointer-events: auto;
        background: rgba(0,0,0,0.7);
        color: #fff;
        padding: 12px 16px;
        border-radius: var(--ui-radius);
        min-width: 160px;
        box-shadow: 0 8px 20px rgba(0,0,0,0.45);
        font-size: var(--ui-font-size);
        line-height: 1;
      }
      .ui-score .label { display:block; font-size: var(--ui-small-size); opacity:0.9; }
      .ui-score .value { font-size: 28px; font-weight:700; margin-top:8px; }

      /* Game Over overlay (center) */
      .ui-gameover {
        position: absolute;
        inset: 0;
        display: none;
        align-items: center;
        justify-content: center;
        pointer-events: auto;
        background: rgba(0,0,0,0.55);
        backdrop-filter: blur(2px);
      }
      .ui-gameover.visible {
        display: flex;
      }
      .ui-gameover .panel {
        max-width: min(720px, 92vw);
        background: rgba(0,0,0,0.78);
        border: 1px solid rgba(255,255,255,0.08);
        border-radius: 16px;
        box-shadow: 0 18px 44px rgba(0,0,0,0.65);
        color: #fff;
        padding: 18px 18px 16px;
        text-align: center;
      }
      .ui-gameover .title {
        font-size: 40px;
        font-weight: 900;
        letter-spacing: 1px;
        margin-bottom: 10px;
      }
      .ui-gameover .subtitle {
        font-size: var(--ui-font-size);
        opacity: 0.92;
      }
      .ui-gameover .subtitle .muted {
        opacity: 0.8;
        font-size: var(--ui-small-size);
        display:block;
        margin-top: 6px;
      }
    `;
    document.head.appendChild(style);
  }

  _createElements() {
    // container overlay
    this.container = document.createElement("div");
    this.container.className = "ui-overlay";

    // score box (top-left)
    this.scoreBox = document.createElement("div");
    this.scoreBox.className = "ui-score";
    this.scoreBox.innerHTML = `<div class="label">Score</div><div class="value">0</div>`;
    this.container.appendChild(this.scoreBox);
    this.scoreValueEl = this.scoreBox.querySelector(".value");

    // game over overlay (center)
    this.gameOverOverlay = document.createElement("div");
    this.gameOverOverlay.className = "ui-gameover";
    this.gameOverOverlay.innerHTML = `
      <div class="panel">
        <div class="title">GAME OVER</div>
        <div class="subtitle">
          Too many marbles reached the end.
          <span class="muted"></span>
        </div>
        <div class="subtitle">
          Final score: <span class="final-score">0</span>
        </div>
      </div>
    `;
    this.gameOverSubtitleEl = this.gameOverOverlay.querySelector(".muted");
    this.gameOverFinalScoreEl =
      this.gameOverOverlay.querySelector(".final-score");
    this.container.appendChild(this.gameOverOverlay);

    this.parent.appendChild(this.container);
  }

  // set score value explicitly
  setScore(n) {
    if (!this.scoreValueEl) return;
    this.scoreValueEl.textContent = String(Math.round(n));
  }

  setGameOver(isGameOver, opts = {}) {
    if (!this.gameOverOverlay) return;
    const on = !!isGameOver;
    this.gameOverOverlay.classList.toggle("visible", on);

    if (this.gameOverSubtitleEl) {
      const reached =
        typeof opts.marblesReachedEnd === "number"
          ? opts.marblesReachedEnd
          : null;
      const threshold =
        typeof opts.threshold === "number" ? opts.threshold : 10;

      if (reached != null) {
        this.gameOverSubtitleEl.textContent = `Reached end: ${reached}/${threshold}`;
      } else {
        this.gameOverSubtitleEl.textContent = `Reached end: ${threshold}/${threshold}`;
      }
    }

    if (this.gameOverFinalScoreEl) {
      const score =
        typeof opts.score === "number"
          ? opts.score
          : typeof opts.score === "string"
            ? Number(opts.score)
            : null;

      if (score != null && Number.isFinite(score)) {
        this.gameOverFinalScoreEl.textContent = String(Math.round(score));
      }
    }
  }

  // small helper to randomize score 0..300
  randomizeScore() {
    this.setScore(this._randomScore());
  }

  _randomScore() {
    return Math.floor(Math.random() * 301);
  }

  // allow binding to a WebSocketClient instance (it uses .on event API)
  bindWebSocketClient(wsClient) {
    if (!wsClient || typeof wsClient.on !== "function") return;

    // Preferred: listen to already-parsed typed events (e.g. {type:"state", score: ...})
    wsClient.on("state", (state) => {
      if (!state) return;
      if (typeof state.score === "number") {
        this.setScore(state.score);
      }

      // server-driven game over
      if (typeof state.game_over === "boolean") {
        this.setGameOver(state.game_over, {
          marblesReachedEnd:
            typeof state.marbles_reached_end === "number"
              ? state.marbles_reached_end
              : typeof state.marbles_reached_end === "string"
                ? Number(state.marbles_reached_end)
                : null,
          threshold: 10,
          score: state.score,
        });
      }
    });

    // Fallback: some clients expose only a generic "message" event
    wsClient.on("message", (data) => {
      if (!data) return;

      // If server sends a full game state as {type:"state", score: ...}
      if (data.type === "state") {
        if (typeof data.score === "number") {
          this.setScore(data.score);
        }
        if (typeof data.game_over === "boolean") {
          this.setGameOver(data.game_over, {
            marblesReachedEnd:
              typeof data.marbles_reached_end === "number"
                ? data.marbles_reached_end
                : typeof data.marbles_reached_end === "string"
                  ? Number(data.marbles_reached_end)
                  : null,
            threshold: 10,
            score: data.score,
          });
        }
        return;
      }

      // server could also send score updates as {type:"score", value: ...}
      if (data.type === "score") {
        this.setScore(data.value || 0);
      }
    });

    wsClient.on("welcome", (_data) => {
      // Welcome messages are optional; don't randomize score because we want server-authoritative score.
      // If you re-enable chat later, you can uncomment:
      // this.addChatMessage("Server", "Welcome — your token was restored.");
    });

    // allow the UI to forward text messages to server as {type:"chat", text}
    this.onSendChat =
      this.onSendChat ||
      ((txt) => {
        if (wsClient && typeof wsClient.send === "function") {
          wsClient.send({ type: "chat", text: txt });
        }
      });
  }
}
