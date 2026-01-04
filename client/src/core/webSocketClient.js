export class WebSocketClient {
  constructor(url, options = {}) {
    this.url = url;
    this.options = {
      reconnectInterval: 1000,
      maxReconnectAttempts: 5,
      heartbeatInterval: 30000,
      ...options,
    };
    this.reconnectAttempts = 0;
    this.messageQueue = [];
    this.eventHandlers = {};
    this.isConnected = false;

    // read token from localStorage (if present)
    this.clientTokenKey = this.options.clientTokenKey || "zuma_token";
    this.token = localStorage.getItem(this.clientTokenKey);

    this.connect();
  }

  connect() {
    console.log(`Connecting to ${this.url}...`);

    try {
      this.ws = new WebSocket(this.url);
      this.setupEventHandlers();
    } catch (error) {
      console.error("Failed to create WebSocket:", error);
      this.scheduleReconnect();
    }
  }

  setupEventHandlers() {
    this.ws.onopen = (event) => {
      console.log("WebSocket connected");
      this.isConnected = true;
      this.reconnectAttempts = 0;

      // Initialize activity tracker immediately on connect
      this.lastPong = Date.now();

      // Immediately send join with token (if any)
      const joinMsg = {
        type: "join",
        token: this.token || null,
      };
      this.send(joinMsg);

      while (this.messageQueue.length > 0) {
        const message = this.messageQueue.shift();
        this.send(message);
      }

      this.startHeartbeat();
      this.trigger("open", event);
    };

    this.ws.onmessage = (event) => {
      // Try to parse JSON messages
      let data = event.data;
      try {
        data = JSON.parse(event.data);
      } catch (e) {
        // Not JSON, use as-is
      }

      // Handle welcome: store server-issued token and player info
      if (data && data.type === "welcome") {
        if (data.token) {
          this.token = data.token;
          try {
            localStorage.setItem(this.clientTokenKey, this.token);
          } catch (e) {
            console.warn("Failed to persist token:", e);
          }
        }
        this.trigger("welcome", data);
        return;
      }

      // Handle ping/pong for heartbeat
      if (data && data.type === "pong") {
        this.lastPong = Date.now();
        return;
      }

      // Trigger custom message handlers
      this.trigger("message", data);

      // Trigger typed message handlers
      if (data && data.type) {
        this.trigger(data.type, data);
      }
    };

    this.ws.onerror = (error) => {
      console.error("WebSocket error:", error);
      this.trigger("error", error);
    };

    this.ws.onclose = (event) => {
      console.log(`WebSocket closed: ${event.code} - ${event.reason}`);
      this.isConnected = false;
      this.stopHeartbeat();

      // Trigger custom close handlers
      this.trigger("close", event);

      // Attempt to reconnect if not a normal closure
      if (event.code !== 1000 && event.code !== 1001) {
        this.scheduleReconnect();
      }
    };
  }

  send(message) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      const data =
        typeof message === "object" ? JSON.stringify(message) : message;
      this.ws.send(data);
    } else {
      // Queue message if not connected
      console.log("WebSocket not connected, queuing message");
      this.messageQueue.push(message);
    }
  }

  startHeartbeat() {
    this.stopHeartbeat();
    this.lastPong = Date.now();

    this.heartbeatTimer = setInterval(() => {
      if (!this.ws && this.ws.readyState === WebSocket.OPEN) return;

      this.send({ type: "ping", timestamp: Date.now() });

      const timeSinceLastPong = Date.now() - this.lastPong;

      // e.g. allow 90s silence for a 30s ping
      if (timeSinceLastPong > this.options.heartbeatInterval * 3) {
        console.warn("Heartbeat timeout, forcing reconnect...");
        this.ws.close(4000, "Heartbeat timeout");
      }
    }, this.options.heartbeatInterval);
  }

  stopHeartbeat() {
    if (this.heartbeatTimer) clearInterval(this.heartbeatTimer);
    if (this.pongCheckTimeout) clearTimeout(this.pongCheckTimeout);
    this.heartbeatTimer = null;
    this.pongCheckTimeout = null;
  }

  scheduleReconnect() {
    if (this.reconnectAttempts >= this.options.maxReconnectAttempts) {
      console.error("Max reconnection attempts reached");
      this.trigger("maxReconnectAttemptsReached");
      return;
    }

    this.reconnectAttempts++;
    const delay =
      this.options.reconnectInterval * Math.pow(2, this.reconnectAttempts - 1);
    console.log(
      `Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})...`,
    );

    setTimeout(() => {
      this.connect();
    }, delay);
  }

  on(event, handler) {
    if (!this.eventHandlers[event]) {
      this.eventHandlers[event] = [];
    }
    this.eventHandlers[event].push(handler);
  }

  off(event, handler) {
    if (this.eventHandlers[event]) {
      this.eventHandlers[event] = this.eventHandlers[event].filter(
        (h) => h !== handler,
      );
    }
  }

  trigger(event, data) {
    if (this.eventHandlers[event]) {
      this.eventHandlers[event].forEach((handler) => {
        try {
          handler(data);
        } catch (error) {
          console.error(`Error in ${event} handler:`, error);
        }
      });
    }
  }

  close() {
    this.reconnectAttempts = this.options.maxReconnectAttempts;
    this.stopHeartbeat();
    if (this.ws) {
      this.ws.close(1000, "Client closing connection");
    }
  }
}
