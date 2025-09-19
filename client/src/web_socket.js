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

      // Send any queued messages
      while (this.messageQueue.length > 0) {
        const message = this.messageQueue.shift();
        this.send(message);
      }

      // Start heartbeat
      this.startHeartbeat();

      // Trigger custom open handlers
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
    this.heartbeatTimer = setInterval(() => {
      if (this.ws.readyState === WebSocket.OPEN) {
        this.send({ type: "ping", timestamp: Date.now() });

        // Check for pong timeout
        setTimeout(() => {
          const timeSinceLastPong = Date.now() - (this.lastPong || 0);
          if (timeSinceLastPong > this.options.heartbeatInterval * 2) {
            console.log("Heartbeat timeout, reconnecting...");
            this.ws.close();
          }
        }, 5000);
      }
    }, this.options.heartbeatInterval);
  }

  stopHeartbeat() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
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
