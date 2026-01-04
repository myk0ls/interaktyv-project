# Client-Side Interpolation System

## Overview

This document describes the client-side interpolation system implemented to make ball and player movement appear smooth, even when the server only sends updates at a fixed tickrate (e.g., 20 or 60 ticks per second).

## Problem

Without interpolation, entities would "snap" or "teleport" between positions as updates arrive from the server, creating choppy visual movement that's limited by the server's tickrate.

## Solution

We've implemented **linear interpolation (lerp)** on the client side. Instead of immediately setting an entity's position to the server's value, we:

1. Store the server's position as a **target position**
2. Maintain a **current position** that smoothly moves toward the target
3. Every frame, interpolate between current and target positions

## Implementation Details

### MarbleRenderer (`marbleRenderer.js`)

Each marble now stores:
- `mesh`: The Three.js mesh object
- `targetPos`: The authoritative position from the server
- `currentPos`: The interpolated position used for rendering
- `velocity`: Optional velocity vector for future use

**Key settings:**
```javascript
this.interpolationSpeed = 10; // Higher = snappier, lower = smoother
```

**Method:**
- `update(marblesArray)`: Called when server state arrives, updates target positions
- `interpolate(dt)`: Called every frame, smoothly moves current positions toward targets

### PlayerRenderer (`playerRenderer.js`)

Each player now stores:
- `mesh`: The Three.js mesh/group object
- `preview`: The preview marble mesh
- `targetPos`: The authoritative position from the server
- `currentPos`: The interpolated position used for rendering
- `targetYaw`: The authoritative rotation from the server
- `currentYaw`: The interpolated rotation used for rendering

**Key settings:**
```javascript
this.interpolationSpeed = 10; // Position interpolation speed
this.rotationSpeed = 8;       // Rotation interpolation speed
```

**Method:**
- `update(playersArray)`: Called when server state arrives, updates target positions and rotations
- `interpolate(dt)`: Called every frame, smoothly interpolates both position and rotation

**Special handling for rotation:**
The rotation interpolation includes angle wrapping to ensure the player rotates via the shortest path (e.g., from 350° to 10° goes through 360°/0° instead of backwards through 180°).

## Usage

The `SceneManager` class automatically calls both interpolation methods every frame:

```javascript
update(dt, gameState) {
  if (gameState && gameState.players) {
    this.playerRenderer.update(gameState.players);
  }
  if (gameState && gameState.marbles) {
    this.marbleRenderer.update(gameState.marbles);
  }
  
  // Interpolate every frame for smooth movement
  this.playerRenderer.interpolate(dt);
  this.marbleRenderer.interpolate(dt);
}
```

## Tuning

### Interpolation Speed

The `interpolationSpeed` parameter controls how quickly entities catch up to their target positions:

- **Higher values (15-20)**: More responsive, snappier movement, but may look jittery if server updates are inconsistent
- **Lower values (5-8)**: Smoother, more fluid movement, but entities lag slightly behind their "true" server position
- **Recommended: 10**: Good balance between smoothness and responsiveness

### Rotation Speed

For players, `rotationSpeed` controls how quickly they rotate:

- **Higher values (12-15)**: Quick, snappy turns
- **Lower values (5-8)**: Smooth, gradual rotation
- **Recommended: 8**: Natural-looking rotation speed

## Math Explained

The interpolation uses a simple lerp formula:

```
currentPos = currentPos + (targetPos - currentPos) * alpha
```

Where `alpha = min(1, interpolationSpeed * deltaTime)`

This creates an exponential ease-out effect:
- The entity moves quickly when far from the target
- It slows down as it approaches the target
- It never overshoots (alpha is clamped to 1)

## Frame Rate Independence

The interpolation is **frame rate independent** because it uses `dt` (delta time):
- At 60 FPS: Smaller alpha values, many small steps
- At 30 FPS: Larger alpha values, fewer but larger steps
- Result: Same visual smoothness regardless of frame rate

## Benefits

1. **Smooth visuals**: Movement appears fluid at any client frame rate
2. **Network efficiency**: Server can send updates at lower rates without affecting visual quality
3. **Reduced bandwidth**: Fewer position updates needed from server
4. **Better player experience**: Movement feels responsive and natural

## Future Enhancements

Potential improvements to consider:

1. **Prediction**: Use velocity to predict where entities will be
2. **Extrapolation**: Continue movement beyond last known position
3. **Dead reckoning**: Predict movement based on last known velocity/input
4. **Adaptive interpolation**: Adjust speed based on network latency
5. **Cubic interpolation**: Use spline curves for even smoother paths

## Testing

To verify interpolation is working:

1. Lower the server tickrate (e.g., to 10 TPS)
2. Movement should still appear smooth
3. Entities should not "snap" between positions
4. Adjust `interpolationSpeed` to find your preferred feel

## Notes

- Interpolation adds a small amount of latency (typically 50-100ms)
- This is acceptable for non-player entities (marbles)
- For local player movement, consider using **client-side prediction** instead
- The system gracefully handles new entities (they appear instantly at their first position)