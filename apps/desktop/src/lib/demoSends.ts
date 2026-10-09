// Simulated Fuse Send for the browser demo, mirroring sends.rs: a file is prepared
// (read twice), then shared behind a link; a link is looked up, then received and
// checked. URL parameters: sends=1 (a share and a received file at start).
// A link containing "offline" never finds its sender.
import type { ReceiveView, ShareView, UiError, UiEvent } from './types'
import { isSendLink } from './sendLink'

const MB = 1024 * 1024
const PAGE = 'https://arshpunisher.github.io/fuselane/s#'

function err(code: string, message: string, hint: string | null): UiError {
  return { code, message, hint }
}

function token(seed: string): string {
  let h = 0x811c9dc5
  let out = 'v1.'
  for (let round = 0; round < 6; round++) {
    for (const c of seed + round) h = Math.imul(h ^ c.charCodeAt(0), 0x01000193) >>> 0
    out += h.toString(36)
  }
  return out
}

export function createDemoSends(params: URLSearchParams, emit: () => (e: UiEvent) => void) {
  const shares: ShareView[] = []
  const receives: (ReceiveView & { wait: number })[] = []
  let next = 1

  if (params.get('sends') === '1') {
    shares.push({
      id: 'a1b2c3d4e5f60718293a4b5c6d7e8f9012345678',
      name: 'Wedding photos.zip',
      size: 2.4 * 1024 * MB,
      link: PAGE + token('wedding'),
      state: 'sharing',
      prepared: 1,
      sent: 812 * MB,
      peers: 1,
      once: false,
      error: null,
    })
    receives.push({
      id: 'ffeeddccbbaa99887766554433221100ffeeddcc',
      name: 'Band demo.wav',
      size: 86 * MB,
      done: 86 * MB,
      state: 'done',
      path: '/Users/demo/Downloads/Band demo.wav',
      error: null,
      wait: 0,
    })
  }

  const send = () =>
    emit()({ type: 'sends', shares: shares.map((s) => ({ ...s })), receives: receives.map(view) })

  function view(r: ReceiveView & { wait: number }): ReceiveView {
    const { wait: _w, ...rest } = r
    return rest
  }

  function step(dt: number) {
    let changed = false
    for (const s of shares) {
      if (s.state === 'sharing' && s.peers > 0) {
        s.sent = Math.min(s.size, s.sent + 160 * MB * dt)
        if (s.once && s.sent >= s.size) {
          s.state = 'sent'
          s.link = null
          s.peers = 0
        }
        changed = true
      }
      if (s.state === 'preparing') {
        s.prepared = Math.min(1, s.prepared + dt / 2)
        if (s.prepared >= 1) {
          s.state = 'sharing'
          s.id = `share${next++}`.padEnd(40, '0')
          s.link = PAGE + token(s.name)
        }
        changed = true
      }
    }
    for (const r of receives) {
      if (r.state === 'finding') {
        r.wait -= dt
        if (r.wait <= 0) {
          if (r.id.includes('offline')) {
            r.state = 'failed'
            r.error =
              "Fuselane couldn't reach the sender. Ask them to open Fuselane and keep it open until the file arrives, then try the link again. On different networks, their router may block incoming connections: turning on UPnP there, or using the same Wi-Fi, fixes it."
          } else {
            r.state = 'receiving'
            r.name = 'Receiving…'
            r.size = 340 * MB
          }
        }
        changed = true
      } else if (r.state === 'receiving') {
        r.done = Math.min(r.size, r.done + 75 * MB * dt)
        if (r.done >= r.size) {
          r.state = 'checking'
          r.wait = 0.6
        }
        changed = true
      } else if (r.state === 'checking') {
        r.wait -= dt
        if (r.wait <= 0) {
          r.state = 'done'
          r.name = 'Holiday video.mov'
          r.path = '/Users/demo/Downloads/Holiday video.mov'
        }
        changed = true
      }
    }
    if (changed) send()
  }

  const methods = {
    pickSendFile: async () =>
      params.get('pick') === 'cancel' ? null : '/Users/demo/Movies/Holiday video.mov',
    async sendFile(path: string) {
      const name = path.split(/[\\/]/).pop() ?? path
      if (!name.includes('.'))
        throw err(
          'send-folder',
          'Only single files can be sent for now.',
          'Zip the folder and send the zip.',
        )
      const id = `prep-${next++}`
      shares.unshift({
        id,
        name,
        size: 340 * MB,
        link: null,
        state: 'preparing',
        prepared: 0,
        sent: 0,
        peers: 0,
        once: false,
        error: null,
      })
      send()
      return id
    },
    sendsState: async (): Promise<[ShareView[], ReceiveView[]]> => [
      shares.map((s) => ({ ...s })),
      receives.map(view),
    ],
    async sendOnce(id: string, on: boolean) {
      const s = shares.find((x) => x.id === id)
      if (!s) throw err('not-found', 'That share is no longer in the list.', null)
      s.once = on
      send()
    },
    async stopSend(id: string) {
      const i = shares.findIndex((s) => s.id === id)
      if (i < 0) throw err('not-found', 'That share is no longer in the list.', null)
      shares.splice(i, 1)
      send()
    },
    async receiveLink(link: string, _dir: string | null) {
      const text = link.trim()
      if (!isSendLink(text) || text.length < PAGE.length + 8)
        throw err(
          'bad-send-link',
          "That isn't a whole Fuse Send link. Copy all of it and paste it again.",
          `Fuse Send links start with ${PAGE}.`,
        )
      const id = text.includes('offline') ? `offline${next++}` : `recv${next++}`
      receives.unshift({
        id,
        name: 'Looking for the sender…',
        size: 0,
        done: 0,
        state: 'finding',
        path: null,
        error: null,
        wait: 1.2,
      })
      send()
      return id
    },
    async dismissReceive(id: string) {
      const i = receives.findIndex((r) => r.id === id)
      if (i >= 0) receives.splice(i, 1)
      send()
    },
    async revealReceived(id: string) {
      if (!receives.some((r) => r.id === id && r.path))
        throw err('not-found', 'That file is no longer in the list.', null)
    },
  }

  return { methods, step, send }
}
