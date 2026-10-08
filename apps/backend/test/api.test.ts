import { SELF, env } from 'cloudflare:test'
import { describe, expect, it } from 'vitest'

const DEVICE = 'a'.repeat(64)
const OTHER = 'b'.repeat(64)
const base = 'https://fuselane.test'

function call(path: string, init: RequestInit & { device?: string | null } = {}) {
  const headers = new Headers(init.headers)
  const device = init.device === undefined ? DEVICE : init.device
  if (device) headers.set('Authorization', `Device ${device}`)
  if (init.body) headers.set('Content-Type', 'application/json')
  return SELF.fetch(`${base}${path}`, { ...init, headers, redirect: 'manual' })
}

async function create(body: Record<string, unknown>, device?: string | null) {
  return call('/v1/uploads', { method: 'POST', body: JSON.stringify(body), device })
}

async function json(res: Response) {
  return (await res.json()) as Record<string, any>
}

describe('starting an upload', () => {
  it('plans parts and starts the multipart upload', async () => {
    const res = await create({ name: 'video.mov', size: 40 * 1024 * 1024, mime: 'video/quicktime' })
    expect(res.status).toBe(201)
    const b = await json(res)
    expect(b.partSize).toBe(16 * 1024 * 1024)
    expect(b.partCount).toBe(3)
    expect(b.uploadId).toMatch(/^[0-9A-Za-z]{22}$/)
  })

  it('refuses requests with no device key, with words to act on', async () => {
    const res = await create({ name: 'a', size: 1 }, null)
    expect(res.status).toBe(401)
    expect((await json(res)).error).toMatchObject({
      code: 'no-device',
      hint: 'Sign in again from the app.',
    })
    const bad = await call('/v1/uploads', {
      method: 'POST',
      body: '{}',
      headers: { Authorization: 'Device nothex' },
      device: null,
    })
    expect(bad.status).toBe(401)
  })

  it('says how big is too big (413) and what is wrong with a body (400)', async () => {
    const big = await create({ name: 'huge.iso', size: 62 * 1024 ** 3 })
    expect(big.status).toBe(413)
    expect((await json(big)).error.message).toBe(
      'This file is 62.0 GB. Free links allow up to 50.0 GB.',
    )
    for (const [body, code] of [
      [{ name: 'a', size: 0 }, 'bad-size'],
      [{ name: 'a', size: -5 }, 'bad-size'],
      [{ name: 'a', size: 1.5 }, 'bad-size'],
      [{ name: 'a', size: '10' }, 'bad-size'],
      [{ name: '', size: 10 }, 'bad-name'],
      [{ name: 'x'.repeat(256), size: 10 }, 'bad-name'],
      [{ name: 'a', size: 10, expiresDays: 3 }, 'bad-expiry'],
    ] as const) {
      const res = await create(body)
      expect(res.status, JSON.stringify(body)).toBe(400)
      expect((await json(res)).error.code).toBe(code)
    }
    const notJson = await call('/v1/uploads', { method: 'POST', body: 'not json' })
    expect(notJson.status).toBe(400)
  })

  it('keeps folders and control characters out of the shared name', async () => {
    const res = await create({ name: '../../etc/pass\nwd', size: 10 })
    const { uploadId } = await json(res)
    const row = await env.DB.prepare('SELECT name, object_key FROM uploads WHERE id = ?1')
      .bind(uploadId)
      .first<{ name: string; object_key: string }>()
    expect(row?.name).toBe('.._.._etc_pass_wd')
    expect(row?.object_key).toBe(`u/${uploadId}/.._.._etc_pass_wd`)
  })
})

describe('parts, completion and the share link', () => {
  it('presigning needs R2 credentials and says so when missing (503)', async () => {
    const { uploadId } = await json(await create({ name: 'a.bin', size: 10 }))
    const res = await call(`/v1/uploads/${uploadId}/parts`, { method: 'POST', body: '{}' })
    expect(res.status).toBe(503)
    expect(res.headers.get('Retry-After')).toBe('300')
    expect((await json(res)).error.code).toBe('not-configured')
  })

  it('only the device that started an upload can touch it', async () => {
    const { uploadId } = await json(await create({ name: 'a.bin', size: 10 }))
    const res = await call(`/v1/uploads/${uploadId}/complete`, {
      method: 'POST',
      body: '{"parts":[]}',
      device: OTHER,
    })
    expect(res.status).toBe(403)
    expect((await call('/v1/uploads/nope/complete', { method: 'POST', body: '{}' })).status).toBe(
      404,
    )
  })

  it('completes from uploaded parts, then serves a share page until it expires', async () => {
    const data = new TextEncoder().encode('hello from two networks')
    const { uploadId } = await json(
      await create({ name: 'hello.txt', size: data.length, mime: 'text/plain' }),
    )
    // Stand in for the app: upload the one part straight to storage.
    const row = await env.DB.prepare('SELECT object_key, r2_upload_id FROM uploads WHERE id = ?1')
      .bind(uploadId)
      .first<{ object_key: string; r2_upload_id: string }>()
    const mpu = env.UPLOADS.resumeMultipartUpload(row!.object_key, row!.r2_upload_id)
    const part = await mpu.uploadPart(1, data)

    const wrong = await call(`/v1/uploads/${uploadId}/complete`, {
      method: 'POST',
      body: JSON.stringify({ parts: [{ n: 2, etag: part.etag }] }),
    })
    expect(wrong.status).toBe(400)

    const done = await call(`/v1/uploads/${uploadId}/complete`, {
      method: 'POST',
      body: JSON.stringify({ parts: [{ n: 1, etag: part.etag }] }),
    })
    expect(done.status, await done.clone().text()).toBe(200)
    const { shareUrl } = await json(done)
    const slug = new URL(shareUrl).pathname.split('/').pop()!
    expect(slug).toMatch(/^[0-9A-Za-z]{22}$/)
    expect(await (await env.UPLOADS.get(row!.object_key))?.text()).toBe('hello from two networks')

    // Completing twice is a conflict, not a second share.
    const again = await call(`/v1/uploads/${uploadId}/complete`, {
      method: 'POST',
      body: JSON.stringify({ parts: [{ n: 1, etag: part.etag }] }),
    })
    expect(again.status).toBe(409)

    const pageRes = await SELF.fetch(`${base}/s/${slug}`)
    expect(pageRes.status).toBe(200)
    const html = await pageRes.text()
    expect(html).toContain('hello.txt')
    expect(html).toContain('noindex')

    // Expired: 410 with when, for both the page and the file.
    await env.DB.prepare('UPDATE shares SET expires_at = ?1 WHERE slug = ?2')
      .bind(Date.UTC(2025, 9, 12), slug)
      .run()
    const gone = await SELF.fetch(`${base}/s/${slug}`)
    expect(gone.status).toBe(410)
    expect(await gone.text()).toContain('expired on 12 Oct 2025')
    const goneFile = await SELF.fetch(`${base}/s/${slug}/file`, { redirect: 'manual' })
    expect(goneFile.status).toBe(410)
  })

  it('unknown and malformed share links are a plain 404', async () => {
    for (const slug of ['nope', 'A'.repeat(22), '<script>alert(1)</script>']) {
      const res = await SELF.fetch(`${base}/s/${encodeURIComponent(slug)}`)
      expect(res.status).toBe(404)
      expect(await res.text()).not.toContain('<script>alert')
    }
  })

  it('share pages escape the file name', async () => {
    const { uploadId } = await json(
      await create({ name: '<img src=x onerror=alert(1)>.txt', size: 3 }),
    )
    const row = await env.DB.prepare('SELECT object_key, r2_upload_id FROM uploads WHERE id = ?1')
      .bind(uploadId)
      .first<{ object_key: string; r2_upload_id: string }>()
    const part = await env.UPLOADS.resumeMultipartUpload(
      row!.object_key,
      row!.r2_upload_id,
    ).uploadPart(1, 'abc')
    const { shareUrl } = await json(
      await call(`/v1/uploads/${uploadId}/complete`, {
        method: 'POST',
        body: JSON.stringify({ parts: [{ n: 1, etag: part.etag }] }),
      }),
    )
    const html = await (await SELF.fetch(`${base}${new URL(shareUrl).pathname}`)).text()
    expect(html).not.toContain('<img src=x')
    expect(html).toContain('&#60;img')
  })
})
