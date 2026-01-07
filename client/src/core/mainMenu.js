// Mock MainMenu overlay for MarbleChain
// - Creates a centered menu with game title and a list of rooms (mock).
// - Exported class has show()/hide()/setRooms()/onJoin(callback) methods.
// - This file is intentionally independent from UIHandler so you can show/hide it
//   separately (e.g., before joining the scene).
//
// Example usage (in your GameClient / bootstrap):
// import MainMenu from "./MainMenu.js";
// const menu = new MainMenu();
// menu.onJoin((room) => {
//   console.log("Join requested:", room);
//   // hide menu and proceed to join network / scene
//   menu.hide();
//   networkClient.send({ type: "join_room", roomId: room.id });
// });
// menu.show();
export default class MainMenu {
  constructor(opts = {}) {
    this.parent = opts.parent || document.body;
    this.networkClient = opts.networkClient || null;
    this.onJoinCallback = null;

    // Level selection
    this.levels =
      Array.isArray(opts.levels) && opts.levels.length
        ? opts.levels.slice()
        : ["first-level", "second-level"];
    this.selectedLevel = opts.selectedLevel || this.levels[0];

    this._injectStyles();
    this._buildDOM();

    // Start with empty room list
    this.setRooms([]);

    // If network client is provided, request room list
    if (this.networkClient) {
      this._setupNetworkHandlers();
      this.networkClient.listRooms();
    }
  }

  _injectStyles() {
    if (document.getElementById("main-menu-styles")) return;
    const s = document.createElement("style");
    s.id = "main-menu-styles";
    s.textContent = `
      .mc-menu-overlay {
        position: fixed;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        background: rgba(6, 10, 14, 0.55);
        z-index: 10050; /* above UI overlay (9999) */
        pointer-events: auto;
        font-family: system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial;
      }
      .mc-menu {
        width: min(780px, 92vw);
        max-height: 86vh;
        overflow: auto;
        background: linear-gradient(180deg, rgba(255,255,255,0.03), rgba(255,255,255,0.01));
        border-radius: 14px;
        padding: 22px;
        box-shadow: 0 18px 60px rgba(2,6,23,0.6), inset 0 1px 0 rgba(255,255,255,0.02);
        color: #eaeef3;
        border: 1px solid rgba(255,255,255,0.04);
      }
      .mc-header {
        display:flex;
        align-items:baseline;
        justify-content:space-between;
        gap: 16px;
      }
      .mc-title {
        font-size:28px;
        font-weight:800;
        letter-spacing:0.6px;
        margin:0;
      }
      .mc-sub {
        font-size:13px;
        opacity:0.85;
        margin-top:6px;
        color:#dfe7ee;
      }
      .mc-controls {
        display:flex;
        gap:8px;
        align-items:center;
      }
      .mc-btn {
        background:#1e90ff;
        color:white;
        border:none;
        padding:10px 14px;
        border-radius:8px;
        font-weight:700;
        cursor:pointer;
      }
      .mc-btn.ghost {
        background: rgba(255,255,255,0.04);
        color: #dfe7ee;
        border: 1px solid rgba(255,255,255,0.03);
      }

      .mc-level {
        display:flex;
        align-items:center;
        gap:8px;
        margin-top: 14px;
      }
      .mc-level label {
        font-size: 13px;
        opacity: 0.9;
        color: #dfe7ee;
      }
      .mc-level select {
        appearance: none;
        background: rgba(0,0,0,0.22);
        color: #eaeef3;
        border: 1px solid rgba(255,255,255,0.06);
        border-radius: 10px;
        padding: 9px 12px;
        font-weight: 700;
        cursor: pointer;
        outline: none;
        min-width: 220px;
      }

      .mc-rooms {
        margin-top: 18px;
        display:flex;
        flex-direction:column;
        gap:10px;
      }
      .mc-room {
        display:flex;
        align-items:center;
        justify-content:space-between;
        gap:12px;
        padding:12px 14px;
        border-radius:10px;
        background: rgba(0,0,0,0.12);
        border: 1px solid rgba(255,255,255,0.02);
      }
      .mc-room .meta { display:flex; gap:10px; align-items:center; }
      .mc-room .room-name { font-weight:700; font-size:15px; }
      .mc-room .room-count { font-size:13px; opacity:0.85; color:#dfe7ee; }
      .mc-room .join { background:#13ce66; color:#03220f; border:none; padding:8px 12px; border-radius:8px; font-weight:700; cursor:pointer; }
      .mc-empty { padding:16px; text-align:center; color:#cbd6df; opacity:0.9; }

      .mc-footer { margin-top:16px; display:flex; justify-content:flex-end; gap:8px; align-items:center; }
      @media (max-width:520px) {
        .mc-title { font-size:22px; }
        .mc-btn { padding:8px 10px; }
        .mc-level select { min-width: 180px; }
      }
    `;
    document.head.appendChild(s);
  }

  _buildDOM() {
    // overlay
    this.overlay = document.createElement("div");
    this.overlay.className = "mc-menu-overlay";
    this.overlay.style.display = "none"; // hidden by default

    // main box
    const box = document.createElement("div");
    box.className = "mc-menu";

    // header (title + controls)
    const header = document.createElement("div");
    header.className = "mc-header";

    const titleWrap = document.createElement("div");
    const title = document.createElement("h1");
    title.className = "mc-title";
    title.textContent = "MarbleChain";
    titleWrap.appendChild(title);
    const sub = document.createElement("div");
    sub.className = "mc-sub";
    sub.textContent = "Mock lobby — select a room to join (demo)";
    titleWrap.appendChild(sub);

    const controls = document.createElement("div");
    controls.className = "mc-controls";

    const refreshBtn = document.createElement("button");
    refreshBtn.className = "mc-btn ghost";
    refreshBtn.textContent = "Refresh";
    refreshBtn.addEventListener("click", () => this._onRefresh());

    const createBtn = document.createElement("button");
    createBtn.className = "mc-btn";
    createBtn.textContent = "Create Room";
    createBtn.addEventListener("click", () => this._onCreateRoom());

    controls.appendChild(refreshBtn);
    controls.appendChild(createBtn);

    header.appendChild(titleWrap);
    header.appendChild(controls);

    // Level selection row (before rooms list)
    const levelRow = document.createElement("div");
    levelRow.className = "mc-level";

    const levelLabel = document.createElement("label");
    levelLabel.textContent = "Level:";
    levelLabel.setAttribute("for", "mc-level-select");

    this.levelSelect = document.createElement("select");
    this.levelSelect.id = "mc-level-select";

    this.levels.forEach((lvl) => {
      const opt = document.createElement("option");
      opt.value = lvl;
      opt.textContent = lvl;
      if (lvl === this.selectedLevel) opt.selected = true;
      this.levelSelect.appendChild(opt);
    });

    this.levelSelect.addEventListener("change", () => {
      this.selectedLevel = this.levelSelect.value;
    });

    levelRow.appendChild(levelLabel);
    levelRow.appendChild(this.levelSelect);

    // rooms list
    this.roomsEl = document.createElement("div");
    this.roomsEl.className = "mc-rooms";

    // footer
    const footer = document.createElement("div");
    footer.className = "mc-footer";

    const dismiss = document.createElement("button");
    dismiss.className = "mc-btn ghost";
    dismiss.textContent = "Close";
    dismiss.addEventListener("click", () => this.hide());

    footer.appendChild(dismiss);

    box.appendChild(header);
    box.appendChild(levelRow);
    box.appendChild(this.roomsEl);
    box.appendChild(footer);
    this.overlay.appendChild(box);
    this.parent.appendChild(this.overlay);
  }

  // Setup network event handlers
  _setupNetworkHandlers() {
    if (!this.networkClient) return;

    // Handle room list updates
    this.networkClient.on("rooms_list", (rooms) => {
      this.setRooms(rooms);
    });

    // Handle room created - auto-join it
    this.networkClient.on("room_created", (data) => {
      console.log("Room created:", data);
      // Refresh room list
      this.networkClient.listRooms();
      // Auto-join the new room
      if (this.onJoinCallback) {
        this.onJoinCallback({ id: data.roomId, name: data.name });
      }
    });

    // Handle errors
    this.networkClient.on("error", (error) => {
      console.error("Network error:", error);
      alert(error.message || "An error occurred");
    });
  }

  // Public API
  show() {
    this.overlay.style.display = "flex";
    // Refresh room list when showing menu
    if (this.networkClient) {
      this.networkClient.listRooms();
    }
  }

  hide() {
    this.overlay.style.display = "none";
  }

  // setRooms([{id,name,players,maxPlayers}, ...])
  setRooms(rooms) {
    this.rooms = Array.isArray(rooms) ? rooms.slice() : [];
    this._renderRooms();
  }

  addRoom(room) {
    this.rooms = this.rooms || [];
    this.rooms.push(room);
    this._renderRooms();
  }

  onJoin(cb) {
    this.onJoinCallback = cb;
  }

  // ------ internals ------
  _renderRooms() {
    // clear
    this.roomsEl.innerHTML = "";
    if (!this.rooms || this.rooms.length === 0) {
      const empty = document.createElement("div");
      empty.className = "mc-empty";
      empty.textContent =
        "No rooms available — create one to start a mock game.";
      this.roomsEl.appendChild(empty);
      return;
    }

    this.rooms.forEach((r) => {
      const row = document.createElement("div");
      row.className = "mc-room";

      const meta = document.createElement("div");
      meta.className = "meta";
      const name = document.createElement("div");
      name.className = "room-name";
      name.textContent = r.name || String(r.id);
      const count = document.createElement("div");
      count.className = "room-count";
      count.textContent = `${r.players || 0} / ${r.maxPlayers || 2}`;

      meta.appendChild(name);
      meta.appendChild(count);

      const join = document.createElement("button");
      join.className = "join";
      join.textContent = "Join";
      join.addEventListener("click", () => this._doJoin(r));

      row.appendChild(meta);
      row.appendChild(join);
      this.roomsEl.appendChild(row);
    });
  }

  _doJoin(room) {
    if (this.onJoinCallback) {
      try {
        this.onJoinCallback(room);
      } catch (e) {
        console.error("MainMenu onJoin callback error:", e);
      }
    }
    // hide menu by default after join
    this.hide();
  }

  _onRefresh() {
    // Request fresh room list from server
    if (this.networkClient) {
      this.networkClient.listRooms();
    } else {
      // Fallback: just re-render current rooms
      this.setRooms(this.rooms || []);
    }
  }

  _onCreateRoom() {
    if (this.networkClient) {
      // Prompt user for room name
      const name = prompt("Enter room name:", "My Room");
      if (name) {
        // Create room on server, include selected level
        this.networkClient.createRoom(name, 4, this.selectedLevel);
        // The room_created handler will auto-join it
      }
    } else {
      // Fallback: create a mock room
      const id = `room-${Date.now()}`;
      const room = {
        id,
        name: `Room (mock)`,
        players: 1,
        maxPlayers: 2,
        level: this.selectedLevel,
      };
      this.addRoom(room);
      this._doJoin(room);
    }
  }
}
