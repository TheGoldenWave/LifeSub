//! Desktop acceptance coordinator — runs inside the Tauri WebView.
//!
//! When the app is launched with `--acceptance-scenario <name>`, the Rust
//! core starts a heartbeat HTTP server on a random localhost port.  This
//! module sends a 100 ms heartbeat while the real native ASR Job runs,
//! allowing the Rust core to measure P95 UI drift.
//!
//! The heartbeat port is passed via `window.__LIFESUB_ACCEPTANCE_PORT__`.

const HEARTBEAT_INTERVAL_MS = 100
const MAX_HEARTBEAT_DURATION_MS = 60_000

let heartbeatTimer: ReturnType<typeof setInterval> | null = null
let heartbeatStart = 0

/**
 * Start the 100 ms UI heartbeat loop.
 * Sends POST requests to the Rust heartbeat receiver.
 */
export function startAcceptanceHeartbeat(): void {
  const port = (window as unknown as Record<string, unknown>).__LIFESUB_ACCEPTANCE_PORT__ as number | undefined
  if (!port) {
    // Not in acceptance mode — no heartbeat needed
    return
  }

  heartbeatStart = Date.now()
  const endpoint = `http://127.0.0.1:${port}/heartbeat`

  heartbeatTimer = setInterval(() => {
    // Use sendBeacon for fire-and-forget — avoids blocking the UI thread
    if (navigator.sendBeacon) {
      navigator.sendBeacon(endpoint, JSON.stringify({ ts: Date.now() }))
    } else {
      // Fallback: fire a POST without awaiting the response
      fetch(endpoint, {
        method: 'POST',
        body: JSON.stringify({ ts: Date.now() }),
        headers: { 'Content-Type': 'application/json' },
        keepalive: true,
      }).catch(() => {
        // Silently ignore connection errors — the Rust side tracks timeouts
      })
    }

    // Stop after the maximum duration to avoid infinite loops
    if (Date.now() - heartbeatStart > MAX_HEARTBEAT_DURATION_MS) {
      stopAcceptanceHeartbeat()
    }
  }, HEARTBEAT_INTERVAL_MS)
}

/**
 * Stop the heartbeat loop and clean up.
 */
export function stopAcceptanceHeartbeat(): void {
  if (heartbeatTimer !== null) {
    clearInterval(heartbeatTimer)
    heartbeatTimer = null
  }
}

/**
 * Check whether the current session is running in acceptance mode.
 */
export function isAcceptanceMode(): boolean {
  return (
    (window as unknown as Record<string, unknown>).__LIFESUB_ACCEPTANCE_PORT__ !== undefined
  )
}

/**
 * Get the acceptance scenario name, if running in acceptance mode.
 */
export function getAcceptanceScenario(): string | null {
  return ((window as unknown as Record<string, unknown>).__LIFESUB_ACCEPTANCE_SCENARIO__ as string) ?? null
}