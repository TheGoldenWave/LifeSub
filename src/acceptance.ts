// LifeSub desktop acceptance helpers.
// Loaded by App.tsx when the acceptance URL parameter is present.

const ACCEPTANCE_PARAM = 'acceptance-scenario'

export function getAcceptanceScenario(): string | null {
  if (typeof window === 'undefined') return null
  const params = new URLSearchParams(window.location.search)
  return params.get(ACCEPTANCE_PARAM)
}

export function recordHeartbeat(scenario: string): void {
  const now = performance.now()
  // Heartbeat is recorded via DOM mutation or custom event for the
  // acceptance harness to read. In production, this is consumed by
  // the Tauri backend via invoke.
  if (typeof window !== 'undefined') {
    ;(window as unknown as Record<string, unknown>).__lifesub_acceptance_heartbeat = {
      scenario,
      timestampMs: now,
    }
  }
}