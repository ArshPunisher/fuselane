import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  TooBig,
  clean,
  dispositionName,
  nameFor,
  probe,
  run,
  split,
  type FetchFn,
} from '../lib/fetcher.ts'

// The extension's own downloader (REVIEW-2026-10-11 D), against a fake server.

const FILE = new Uint8Array(5 * 1024 * 1024 + 123).map((_, i) => (i * 31 + 7) & 0xff)

/** A server for FILE: honours ranges unless told not to, can fail some requests. */
function server(
  opts: { ranges?: boolean; failEvery?: number; cutAt?: number } = {},
): FetchFn & { calls: number } {
  let calls = 0
  const f = (async (_url: string, init?: RequestInit) => {
    calls++
    f.calls = calls
    if (init?.signal?.aborted) throw init.signal.reason
    if (opts.failEvery && calls % opts.failEvery === 0) throw new TypeError('network down')
    const range = new Headers(init?.headers).get('range')
    const m = range ? /bytes=(\d+)-(\d+)?/.exec(range) : null
    if (m && opts.ranges !== false) {
      const start = Number(m[1])
      const end = Math.min(FILE.length - 1, m[2] ? Number(m[2]) : FILE.length - 1)
      let body = FILE.slice(start, end + 1)
      if (opts.cutAt && body.length > opts.cutAt) body = body.slice(0, opts.cutAt)
      return new Response(body, {
        status: 206,
        headers: {
          'content-range': `bytes ${start}-${end}/${FILE.length}`,
          'content-disposition': 'attachment; filename="os.iso"',
        },
      })
    }
    return new Response(FILE, { status: 200, headers: { 'content-length': String(FILE.length) } })
  }) as FetchFn & { calls: number }
  f.calls = 0
  return f
}

function sink() {
  const out = new Uint8Array(FILE.length)
  return { out, write: async (pos: number, data: Uint8Array) => void out.set(data, pos) }
}

test('the first byte tells the size, whether ranges work, and the name', async () => {
  const p = await probe('https://e.org/dl?id=1', server())
  assert.deepEqual(p, {
    total: FILE.length,
    ranges: true,
    filename: 'os.iso',
    finalUrl: 'https://e.org/dl?id=1',
  })
  const plain = await probe('https://e.org/os.iso', server({ ranges: false }))
  assert.equal(plain.ranges, false)
  assert.equal(plain.total, FILE.length)
})

test('parts come back together byte for byte', async () => {
  const s = sink()
  const parts = split(FILE.length, 6)
  assert.equal(parts.length, 3) // 2 MiB minimum per part
  const n = await run({
    url: 'u',
    parts,
    fetchFn: server(),
    write: s.write,
    signal: new AbortController().signal,
  })
  assert.equal(n, FILE.length)
  assert.deepEqual(s.out, FILE)
})

test('a flaky server and early-ending answers are retried until every byte is in', async () => {
  const s = sink()
  const n = await run({
    url: 'u',
    parts: split(FILE.length, 4),
    fetchFn: server({ failEvery: 3, cutAt: 700_000 }),
    write: s.write,
    signal: new AbortController().signal,
    tries: 6,
  })
  assert.equal(n, FILE.length)
  assert.deepEqual(s.out, FILE)
})

test('paused parts carry on from where they stopped', async () => {
  const s = sink()
  const parts = split(FILE.length, 3)
  const stop = new AbortController()
  let seen = 0
  await assert.rejects(
    run({
      url: 'u',
      parts,
      fetchFn: server({ cutAt: 300_000 }),
      write: async (pos, data) => {
        s.out.set(data, pos)
        if ((seen += data.length) > 1_000_000) stop.abort(new Error('paused'))
      },
      signal: stop.signal,
    }),
  )
  const kept = parts.reduce((n, p) => n + p.done, 0)
  assert.ok(kept > 0 && kept < FILE.length)
  const again = server()
  await run({
    url: 'u',
    parts,
    fetchFn: again,
    write: s.write,
    signal: new AbortController().signal,
  })
  assert.deepEqual(s.out, FILE)
})

test('a server that ignores ranges is refused rather than mixing up the file', async () => {
  await assert.rejects(
    run({
      url: 'u',
      parts: split(FILE.length, 3),
      fetchFn: server({ ranges: false }),
      write: async () => {},
      signal: new AbortController().signal,
      tries: 1,
    }),
    /answered 200/,
  )
})

test('without ranges one stream is read, and an unknown size past the limit stops', async () => {
  const s = sink()
  assert.equal(
    await run({
      url: 'u',
      parts: null,
      fetchFn: server({ ranges: false }),
      write: s.write,
      signal: new AbortController().signal,
    }),
    FILE.length,
  )
  assert.deepEqual(s.out, FILE)
  await assert.rejects(
    run({
      url: 'u',
      parts: null,
      fetchFn: server({ ranges: false }),
      write: async () => {},
      signal: new AbortController().signal,
      limit: 1_000_000,
    }),
    TooBig,
  )
})

test('file names are taken from the server, safely', () => {
  assert.equal(dispositionName(`attachment; filename*=UTF-8''r%C3%A9sum%C3%A9.pdf`), 'résumé.pdf')
  assert.equal(dispositionName('attachment; filename="../../etc/passwd"'), 'passwd')
  assert.equal(dispositionName(null), null)
  assert.equal(clean('..'), null)
  assert.equal(nameFor('https://e.org/a/b/My%20File.zip?x=1', null), 'My File.zip')
  assert.equal(nameFor('https://e.org/', null), 'download')
  assert.equal(nameFor('https://e.org/x', null, '/Users/me/Downloads/real.iso'), 'real.iso')
})
