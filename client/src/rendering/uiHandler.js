// UIHandler - simple DOM UI overlay for the game.
// - Top-left: Score box (shows a random score 0..300 for now)
// - Top-right: Chat panel with messages and input
// Usage:
//   const ui = new UIHandler({ parent: document.body, onSendChat: text => ws.send({type:'chat', text}) });
//   ui.setScore(123);
//   ui.addChatMessage('Alice', 'hello');
//   ws.on('welcome', d => ui.addChatMessage('Server', 'Welcome!')); // example
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
        background: transparent;
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

      /* Chat (top-right) */
      .ui-chat {
        position: absolute;
        top: var(--ui-gap);
        right: var(--ui-gap);
        width: var(--ui-chat-width);
        max-height: 68vh;
        pointer-events: auto;
        display:flex; flex-direction:column;
        background: rgba(0,0,0,0.5);
        border-radius: var(--ui-radius);
        overflow: hidden;
        box-shadow: 0 10px 32px rgba(0,0,0,0.55);
        font-size: var(--ui-font-size);
      }
      .ui-chat .header { padding:12px 14px; font-weight:700; color:#fff; background: rgba(255,255,255,0.02); display:flex; align-items:center; justify-content:space-between; font-size:var(--ui-small-size); }
      .ui-chat .messages { flex:1; overflow:auto; padding:12px 14px; display:flex; flex-direction:column; gap:10px; }
      .ui-chat .message { background: rgba(255,255,255,0.04); color:#fff; padding:8px 12px; border-radius:8px; font-size:var(--ui-small-size); word-wrap:break-word; }
      .ui-chat .message .meta { font-size:12px; opacity:0.85; margin-bottom:6px; color:#ddd; }
      .ui-chat .input-row { display:flex; gap:10px; padding:12px; border-top: 1px solid rgba(255,255,255,0.03); background: rgba(0,0,0,0.04); }
      .ui-chat input[type="text"] { flex:1; padding:10px 12px; border-radius:8px; border:1px solid rgba(255,255,255,0.06); outline:none; font-size:var(--ui-small-size); background: rgba(255,255,255,0.03); color:#fff; }
      .ui-chat button.send { padding:10px 14px; border-radius:8px; background:#1e90ff; color:#fff; border:none; cursor:pointer; font-weight:700; font-size:var(--ui-small-size); }
      .ui-chat button.send:active { transform: translateY(1px); }

      @media (max-width: 720px) {
        :root { --ui-chat-width: calc(92vw); --ui-font-size: 15px; --ui-small-size: 12px; }
        .ui-score { min-width: 120px; padding: 8px 10px; font-size: 16px; }
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

    // chat (top-right)
    this.chatBox = document.createElement("div");
    this.chatBox.className = "ui-chat";

    const header = document.createElement("div");
    header.className = "header";
    header.innerHTML = `<div>Chat</div><div class="small">Ctrl+Enter to send</div>`;
    this.chatBox.appendChild(header);

    const messages = document.createElement("div");
    messages.className = "messages";
    this.chatMessagesEl = messages;
    this.chatBox.appendChild(messages);

    const inputRow = document.createElement("div");
    inputRow.className = "input-row";
    this.chatInput = document.createElement("input");
    this.chatInput.type = "text";
    this.chatInput.placeholder = "Say something...";
    this.chatInput.addEventListener("keydown", (ev) => {
      if (
        (ev.key === "Enter" && ev.ctrlKey) ||
        (ev.key === "Enter" &&
          ev.shiftKey === false &&
          ev.metaKey === false &&
          ev.altKey === false)
      ) {
        // allow Enter to send (without modifier) for convenience
        ev.preventDefault();
        this._doSendChat();
      }
    });
    inputRow.appendChild(this.chatInput);

    const sendBtn = document.createElement("button");
    sendBtn.className = "send";
    sendBtn.textContent = "Send";
    sendBtn.addEventListener("click", () => this._doSendChat());
    inputRow.appendChild(sendBtn);

    this.chatBox.appendChild(inputRow);
    this.container.appendChild(this.chatBox);

    this.parent.appendChild(this.container);
  }

  // add a chat message (author string, message string)
  addChatMessage(author, text, opts = {}) {
    if (!this.chatMessagesEl) return;
    const el = document.createElement("div");
    el.className = "message";
    const meta = document.createElement("div");
    meta.className = "meta";
    meta.textContent = `${author} • ${new Date().toLocaleTimeString()}`;
    el.appendChild(meta);
    const body = document.createElement("div");
    body.className = "body";
    body.textContent = text;
    el.appendChild(body);

    // append and scroll to bottom
    this.chatMessagesEl.appendChild(el);

    // trim old messages
    while (this.chatMessagesEl.children.length > this.maxMessages) {
      this.chatMessagesEl.removeChild(this.chatMessagesEl.firstChild);
    }

    // smooth scroll
    this.chatMessagesEl.scrollTop = this.chatMessagesEl.scrollHeight;
  }

  // send chat (calls onSendChat callback if provided)
  _doSendChat() {
    const text = this.chatInput.value.trim();
    if (!text) return;
    // local echo
    this.addChatMessage("You", text);
    if (this.onSendChat) {
      try {
        this.onSendChat(text);
      } catch (err) {
        console.error("onSendChat handler error", err);
      }
    }
    this.chatInput.value = "";
    this.chatInput.focus();
  }

  // set score value explicitly
  setScore(n) {
    if (!this.scoreValueEl) return;
    this.scoreValueEl.textContent = String(Math.round(n));
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
    // example: when server sends "chat" typed messages, show them
    wsClient.on("message", (data) => {
      if (!data) return;
      if (data.type === "chat") {
        const author = data.author || "Peer";
        const text = data.text || "";
        this.addChatMessage(author, text);
      }
      // server could also send score updates
      if (data.type === "score") {
        this.setScore(data.value || 0);
      }
    });

    wsClient.on("welcome", (data) => {
      // show a small welcome message
      this.addChatMessage("Server", "Welcome — your token was restored.");
      // show player-specific score example
      this.setScore(this._randomScore());
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
