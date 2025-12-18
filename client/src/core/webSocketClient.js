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

      while (this.messageQueue.length > 0) {
        const message = this.messageQueue.shift();
        this.send(message);
      }

      this.startHeartbeat();
      this.trigger("open", event);
    };

    this.ws.onmessage = (event) => {
      console.log("Message received:", event.data);

      // Try to parse JSON messages
      let data = event.data;
      try {
        data = JSON.parse(event.data);
      } catch (e) {
        // Not JSON, use as-is
      }

      // Handle ping/pong for heartbeat
      if (data.type === "pong") {
        this.lastPong = Date.now();
        return;
      }

      // Trigger custom message handlers
      this.trigger("message", data);

      // Trigger typed message handlers
      if (data.type) {
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

    // 1. Initialize lastPong so the first check doesn't fail
    this.lastPong = Date.now();

    this.heartbeatTimer = setInterval(() => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        this.send({ type: "ping", timestamp: Date.now() });

        // 2. Track this specific timeout so we can clear it if needed
        this.pongCheckTimeout = setTimeout(() => {
          const timeSinceLastPong = Date.now() - this.lastPong;

          // If no pong for 2x interval, the connection is "dead"
          if (timeSinceLastPong > this.options.heartbeatInterval * 2) {
            console.warn("Heartbeat timeout, forcing reconnect...");
            // Use a specific code so onclose knows it was a timeout
            this.ws.close(4000, "Heartbeat timeout");
          }
        }, 5000);
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
