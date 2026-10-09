// Simulated Nearby for the browser demo, mirroring nearby.rs: devices on the
// network, who can see this computer, trusted devices, asking before files
// arrive, and sending with progress. URL parameters:
//   nearby=request  someone asks to send a file a moment after Nearby starts
//   nearby=decline  devices decline what you send
//   nearby=empty    nobody else is on the network
import type { DeviceView, NearbyTransfer, NearbyView, UiError, UiEvent } from './types'

const MB = 1024 * 1024
const EVERYONE_SECONDS = 600

function err(code: string, message: string, hint: string | null): UiError {
  return { code, message, hint }
}

const WORDS = ['amber', 'river', 'candle', 'orbit']

export function createDemoNearby(params: URLSearchParams, emit: () => (e: UiEvent) => void) {
  const mode = params.get('nearby')
  const devices: DeviceView[] =
    mode === 'empty'
      ? []
      : [
          {
            fingerprint: 'A1',
            alias: "Maya's MacBook Air",
            kind: 'desktop',
            model: 'Fuselane (macos)',
            trusted: true,
            fuselane: true,
          },
          {
            fingerprint: 'B2',
            alias: 'STUDIO-PC',
            kind: 'desktop',
            model: 'Fuselane (windows)',
            trusted: true,
            fuselane: true,
          },
          {
            fingerprint: 'C3',
            alias: "Ravi's ThinkPad",
            kind: 'desktop',
            model: 'Fuselane (linux)',
            trusted: false,
            fuselane: true,
          },
          {
            fingerprint: 'D4',
            alias: 'Pixel 9',
            kind: 'mobile',
            model: 'Google',
            trusted: false,
            fuselane: false,
          },
        ]
  const view: NearbyView = {
    on: false,
    me: "Arsh's MacBook Pro",
    everyoneFor: null,
    devices,
    trusted: [
      { fingerprint: 'A1', alias: "Maya's MacBook Air", since: '2026-10-02' },
      { fingerprint: 'B2', alias: 'STUDIO-PC', since: '2026-09-19' },
    ],
    transfers: [],
    request: null,
    problem: null,
  }
  let everyoneUntil = 0
  let next = 1
  const clone = (): NearbyView => structuredClone(view)
  const send = () => {
    view.everyoneFor =
      everyoneUntil > Date.now() ? Math.ceil((everyoneUntil - Date.now()) / 1000) : null
    emit()({ type: 'nearby', view: clone() })
  }

  function progress(t: NearbyTransfer, step: number, onDone: () => void) {
    const timer = setInterval(() => {
      if (t.state === 'cancelled') return clearInterval(timer)
      t.done = Math.min(t.size, t.done + step)
      if (t.done >= t.size) {
        t.state = 'done'
        t.words = null
        clearInterval(timer)
        onDone()
      }
      send()
    }, 200)
  }

  const methods = {
    nearbyStart: async () => {
      if (!view.on) {
        view.on = true
        setInterval(send, 1000)
        if (mode === 'request')
          setTimeout(() => {
            view.request = {
              id: next++,
              alias: "Ravi's ThinkPad",
              kind: 'desktop',
              model: 'Fuselane (linux)',
              files: ['Holiday video.mov'],
              total: 2.4 * 1024 * MB,
              words: WORDS,
              verified: true,
            }
            send()
          }, 600)
      }
      send()
      return clone()
    },
    nearbyState: async () => clone(),
    nearbySetEveryone: async (on: boolean) => {
      everyoneUntil = on ? Date.now() + EVERYONE_SECONDS * 1000 : 0
      send()
      return clone()
    },
    nearbyPick: async () =>
      params.get('pick') === 'cancel' ? [] : ['/Users/demo/Movies/Holiday video.mov'],
    nearbySend: async (fingerprint: string, paths: string[]) => {
      const d = devices.find((x) => x.fingerprint === fingerprint)
      if (!d)
        throw err(
          'nearby-gone',
          "That device isn't on the network anymore.",
          'Ask them to open Fuselane or LocalSend, then try again.',
        )
      if (!paths.length) throw err('send-missing', 'Pick at least one file to send.', null)
      const t: NearbyTransfer = {
        id: `out-${next++}`,
        direction: 'out',
        device: d.alias,
        name: paths.length === 1 ? (paths[0]?.split('/').pop() ?? '') : `${paths.length} files`,
        size: 2.4 * 1024 * MB,
        done: 0,
        state: 'asking',
        error: null,
        words: d.fuselane && !d.trusted ? WORDS : null,
        path: null,
      }
      view.transfers.unshift(t)
      send()
      setTimeout(
        () => {
          if (t.state !== 'asking') return
          if (mode === 'decline' && !d.trusted) {
            t.state = 'declined'
            t.words = null
            return send()
          }
          t.state = 'sending'
          t.words = null
          progress(t, 0.06 * t.size, () => {})
        },
        d.trusted ? 300 : 1200,
      )
      return clone()
    },
    nearbyAnswer: async (id: number, accept: boolean, trust: boolean) => {
      const r = view.request
      if (!r || r.id !== id) throw err('nearby-gone', 'That request has ended.', null)
      view.request = null
      if (accept) {
        if (trust) {
          view.trusted.push({ fingerprint: 'C3', alias: r.alias, since: '2026-10-09' })
          const d = devices.find((x) => x.alias === r.alias)
          if (d) d.trusted = true
        }
        const t: NearbyTransfer = {
          id: `in-${next++}`,
          direction: 'in',
          device: r.alias,
          name: r.files.length === 1 ? (r.files[0] ?? '') : `${r.files.length} files`,
          size: r.total,
          done: 0,
          state: 'receiving',
          error: null,
          words: null,
          path: null,
        }
        view.transfers.unshift(t)
        progress(t, 0.08 * t.size, () => {
          t.path = `/Users/demo/Downloads/${t.name}`
        })
      }
      send()
      return clone()
    },
    nearbyForget: async (fingerprint: string) => {
      view.trusted = view.trusted.filter((t) => t.fingerprint !== fingerprint)
      for (const d of devices) if (d.fingerprint === fingerprint) d.trusted = false
      send()
      return clone()
    },
    nearbyCancel: async (id: string) => {
      const t = view.transfers.find((x) => x.id === id)
      if (t && (t.state === 'asking' || t.state === 'sending')) {
        t.state = 'cancelled'
        t.words = null
      }
      send()
    },
    nearbyClear: async (id: string) => {
      view.transfers = view.transfers.filter((t) => t.id !== id)
      send()
      return clone()
    },
    nearbyReveal: async () => {},
  }
  return { methods, send }
}
