// One line about the app, shared by the popup and the settings page.
import type { AskApp } from './handoff.ts'

export type AppState = 'connected' | 'not-running' | 'not-installed'

export async function appStatus(ask: AskApp): Promise<{ state: AppState; text: string }> {
  try {
    const pong = (await ask({ type: 'ping' })) as { app?: { version?: unknown } } | undefined
    const version = pong?.app?.version
    return typeof version === 'string'
      ? { state: 'connected', text: `Connected to Fuselane ${version}.` }
      : {
          state: 'not-running',
          text: 'Fuselane is installed but not running. Open it to hand downloads over.',
        }
  } catch {
    return {
      state: 'not-installed',
      text: 'Fuselane isn’t installed on this computer, so downloads stay in the browser.',
    }
  }
}
