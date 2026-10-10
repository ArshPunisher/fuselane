// Simulated Nearby for the browser demo, mirroring nearby.rs: devices on the
// network, who can see this computer, trusted devices, asking before files
// arrive, and sending with progress. URL parameters:
//   nearby=request  someone asks to send a file a moment after Nearby starts
//   nearby=decline  devices decline what you send
//   nearby=empty    nobody else is on the network
import type { DeviceView, NearbyTransfer, NearbyView, UiError, UiEvent } from './types'
import type { FileDrop } from './backend'

const MB = 1024 * 1024
const EVERYONE_SECONDS = 600

function err(code: string, message: string, hint: string | null): UiError {
  return { code, message, hint }
}

// A real code for the demo link, made once with a QR library.
const DEMO_QR =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 29 29" shape-rendering="crispEdges"><path fill="#ffffff" d="M0 0h29v29H0z"/><path stroke="#000000" d="M0 0.5h7m1 0h3m1 0h3m1 0h1m1 0h3m1 0h7M0 1.5h1m5 0h1m2 0h1m2 0h4m2 0h1m3 0h1m5 0h1M0 2.5h1m1 0h3m1 0h1m2 0h2m3 0h1m2 0h3m2 0h1m1 0h3m1 0h1M0 3.5h1m1 0h3m1 0h1m1 0h2m1 0h2m3 0h1m1 0h2m2 0h1m1 0h3m1 0h1M0 4.5h1m1 0h3m1 0h1m1 0h1m2 0h1m5 0h1m4 0h1m1 0h3m1 0h1M0 5.5h1m5 0h1m1 0h2m1 0h2m4 0h4m1 0h1m5 0h1M0 6.5h7m1 0h1m1 0h1m1 0h1m1 0h1m1 0h1m1 0h1m1 0h1m1 0h7M8 7.5h1m2 0h1m1 0h1m1 0h4M0 8.5h1m3 0h1m1 0h4m3 0h1m2 0h1m1 0h8m2 0h1M2 9.5h1m1 0h1m5 0h2m1 0h2m2 0h2m1 0h1m1 0h7M0 10.5h1m2 0h5m4 0h1m6 0h2m7 0h1M0 11.5h1m1 0h1m2 0h1m3 0h1m1 0h2m1 0h1m1 0h3m1 0h2m1 0h1m3 0h2M0 12.5h2m1 0h1m2 0h2m5 0h3m1 0h1m1 0h1m1 0h5m1 0h1M5 13.5h1m1 0h2m1 0h1m1 0h1m2 0h1m1 0h4m4 0h4M0 14.5h1m5 0h2m5 0h3m4 0h1m1 0h1m5 0h1M1 15.5h2m1 0h2m1 0h1m2 0h4m2 0h3m1 0h1m2 0h1m2 0h3M4 16.5h3m1 0h1m2 0h1m1 0h1m2 0h1m1 0h1m1 0h4m1 0h1m1 0h1M0 17.5h4m4 0h1m2 0h1m1 0h3m4 0h2m5 0h2M2 18.5h2m2 0h2m1 0h1m1 0h1m9 0h1m2 0h3m1 0h1M4 19.5h1m4 0h2m3 0h1m1 0h5m1 0h1m1 0h5M0 20.5h2m1 0h4m3 0h1m1 0h4m1 0h2m1 0h6m2 0h1M8 21.5h2m2 0h1m3 0h3m1 0h1m3 0h1m1 0h1m1 0h1M0 22.5h7m1 0h1m3 0h4m3 0h2m1 0h1m1 0h3m1 0h1M0 23.5h1m5 0h1m5 0h2m2 0h1m1 0h1m1 0h1m3 0h5M0 24.5h1m1 0h3m1 0h1m1 0h4m1 0h1m2 0h1m1 0h7m2 0h1M0 25.5h1m1 0h3m1 0h1m4 0h1m1 0h2m5 0h1m2 0h4M0 26.5h1m1 0h3m1 0h1m2 0h2m1 0h2m3 0h1m3 0h2m3 0h3M0 27.5h1m5 0h1m3 0h2m4 0h3m3 0h3m2 0h2M0 28.5h7m1 0h5m1 0h2m1 0h1m1 0h9"/></svg>'

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
    phone: null,
  }
  const drops = new Set<(e: FileDrop) => void>()
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
        if (mode === 'text')
          setTimeout(() => {
            view.request = {
              id: next++,
              alias: 'STUDIO-PC',
              kind: 'desktop',
              model: 'Fuselane (windows)',
              files: ['message.txt'],
              total: 22,
              verified: true,
              text: 'ssh studio@192.168.1.9',
            }
            send()
          }, 600)
        if (mode === 'request')
          setTimeout(() => {
            view.request = {
              id: next++,
              alias: "Ravi's ThinkPad",
              kind: 'desktop',
              model: 'Fuselane (linux)',
              files: ['Holiday video.mov'],
              total: 2.4 * 1024 * MB,
              verified: true,
              text: null,
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
        path: null,
        text: null,
      }
      view.transfers.unshift(t)
      send()
      setTimeout(
        () => {
          if (t.state !== 'asking') return
          if (mode === 'decline' && !d.trusted) {
            t.state = 'declined'
            return send()
          }
          t.state = 'sending'
          progress(t, 0.06 * t.size, () => {})
        },
        d.trusted ? 300 : 1200,
      )
      return clone()
    },
    nearbySendText: async (fingerprint: string, text: string | null) => {
      const d = devices.find((x) => x.fingerprint === fingerprint)
      if (!d) throw err('nearby-gone', "That device isn't on the network anymore.", null)
      const body = text ?? 'https://releases.example.org/26.04/ubuntu-26.04-desktop-amd64.iso'
      if (!body.trim())
        throw err('nothing-to-send', "There's no text to send. Copy something first.", null)
      view.transfers.unshift({
        id: `text-${next++}`,
        direction: 'out',
        device: d.alias,
        name: body.split('\n')[0]!.slice(0, 80),
        size: body.length,
        done: body.length,
        state: 'done',
        error: null,
        path: null,
        text: body,
      })
      send()
      return clone()
    },
    nearbyAnswer: async (id: number, accept: boolean, trust: boolean) => {
      const r = view.request
      if (!r || r.id !== id) throw err('nearby-gone', 'That request has ended.', null)
      view.request = null
      if (accept && r.text !== null) {
        view.transfers.unshift({
          id: `text-in-${next++}`,
          direction: 'in',
          device: r.alias,
          name: r.text.split('\n')[0]!.slice(0, 80),
          size: r.text.length,
          done: r.text.length,
          state: 'done',
          error: null,
          path: null,
          text: r.text,
        })
      } else if (accept) {
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
          path: null,
          text: null,
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
      if (t && (t.state === 'asking' || t.state === 'sending' || t.state === 'receiving')) {
        t.state = 'cancelled'
      }
      send()
    },
    nearbyClear: async (id: string) => {
      view.transfers = view.transfers.filter((t) => t.id !== id)
      send()
      return clone()
    },
    nearbyReveal: async () => {},
    onFileDrop: (cb: (e: FileDrop) => void) => {
      // Tests stand in for the OS dragging files over the window.
      drops.add(cb)
      ;(window as unknown as { __demoFileDrop?: (e: FileDrop) => void }).__demoFileDrop = (e) => {
        for (const f of drops) f(e)
      }
      return () => drops.delete(cb)
    },
    nearbyPhone: async (on: boolean) => {
      view.phone = on ? { url: 'http://192.168.1.24:7380/p/7f3c9a', qr: DEMO_QR, offers: [] } : null
      send()
      return clone()
    },
    nearbyPhoneOffer: async (add: string[], remove: string | null) => {
      if (!view.phone) throw err('nearby-phone', 'The phone page is off.', null)
      if (remove) view.phone.offers = view.phone.offers.filter((o) => o.id !== remove)
      for (const p of add)
        view.phone.offers.push({
          id: `o${next++}`,
          name: p.split('/').pop() ?? p,
          size: 212 * 1024,
        })
      send()
      return clone()
    },
  }
  return { methods, send }
}
