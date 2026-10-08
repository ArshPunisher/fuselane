// Fuselane's upload and share-link service (BONDED-UPLOADS.md, ADR 0007).
import { Hono } from 'hono'
import { fail, humanBytes } from './errors'
import { MAX_PARTS, MAX_SIZE, plan } from './plan'
import { credentials, getUrl, partUrl } from './presign'

type Env = {
  DB: D1Database
  UPLOADS: R2Bucket
  PUBLIC_URL: string
  R2_BUCKET: string
  R2_ACCOUNT_ID?: string
  R2_ACCESS_KEY_ID?: string
  R2_SECRET_ACCESS_KEY?: string
}

type Upload = {
  id: string
  device: string
  object_key: string
  r2_upload_id: string
  name: string
  size: number
  mime: string
  part_size: number
  part_count: number
  expires_days: number
  state: 'open' | 'completed' | 'aborted'
}

const app = new Hono<{ Bindings: Env; Variables: { device: string } }>()
const EXPIRY_DAYS = [1, 7, 30]
const PRESIGN_BATCH = 100

function randomId(bytes = 16): string {
  // base62 of 128 random bits: unguessable share slugs and ids.
  const alphabet = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz'
  const b = crypto.getRandomValues(new Uint8Array(bytes))
  let n = 0n
  for (const x of b) n = (n << 8n) | BigInt(x)
  let s = ''
  while (n > 0n) {
    s = alphabet[Number(n % 62n)] + s
    n /= 62n
  }
  return s.padStart(22, '0')
}

/** File names as the receiver will see them: no folders, no control characters. */
function cleanName(raw: unknown): string | null {
  if (typeof raw !== 'string') return null
  const name = raw.replace(/[\u0000-\u001f\u007f/\\]/g, '_').trim()
  return name.length > 0 && name.length <= 255 ? name : null
}

app.onError((err, c) => {
  console.error(err)
  return fail(
    c,
    503,
    'unavailable',
    'Storage is busy or unavailable.',
    'The app retries automatically.',
    30,
  )
})

app.notFound((c) => fail(c, 404, 'not-found', 'There is nothing here.'))

// MVP device auth (6.2 adds keys and quotas): a random device id the app keeps.
app.use('/v1/*', async (c, next) => {
  const auth = c.req.header('Authorization') ?? ''
  const m = /^Device ([0-9a-f]{64})$/.exec(auth)
  if (!m?.[1])
    return fail(
      c,
      401,
      'no-device',
      'This request needs a device key.',
      'Sign in again from the app.',
    )
  c.set('device', m[1])
  await next()
})

app.post('/v1/uploads', async (c) => {
  const body = await c.req.json<Record<string, unknown>>().catch(() => null)
  if (!body || typeof body !== 'object')
    return fail(c, 400, 'bad-request', 'The request body must be JSON.')
  const { size } = body
  if (typeof size !== 'number' || !Number.isSafeInteger(size) || size <= 0)
    return fail(c, 400, 'bad-size', '`size` must be a positive whole number of bytes.')
  if (size > MAX_SIZE)
    return fail(
      c,
      413,
      'too-large',
      `This file is ${humanBytes(size)}. Free links allow up to ${humanBytes(MAX_SIZE)}.`,
      'Split it into smaller files.',
    )
  const name = cleanName(body.name)
  if (!name) return fail(c, 400, 'bad-name', '`name` must be a file name of 1 to 255 characters.')
  const mime =
    typeof body.mime === 'string' && /^[\w.+-]+\/[\w.+-]+$/.test(body.mime)
      ? body.mime
      : 'application/octet-stream'
  const expiresDays = body.expiresDays ?? 7
  if (typeof expiresDays !== 'number' || !EXPIRY_DAYS.includes(expiresDays))
    return fail(c, 400, 'bad-expiry', '`expiresDays` must be 1, 7 or 30.')
  const p = plan(size)
  if (p.partCount > MAX_PARTS)
    return fail(c, 413, 'too-many-parts', 'This file needs more parts than storage allows.')
  const id = randomId()
  const key = `u/${id}/${name}`
  const mpu = await c.env.UPLOADS.createMultipartUpload(key, {
    httpMetadata: { contentType: mime },
  })
  await c.env.DB.prepare(
    `INSERT INTO uploads (id, device, object_key, r2_upload_id, name, size, mime, part_size, part_count, expires_days, state, created_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'open', ?11)`,
  )
    .bind(
      id,
      c.get('device'),
      key,
      mpu.uploadId,
      name,
      size,
      mime,
      p.partSize,
      p.partCount,
      expiresDays,
      Date.now(),
    )
    .run()
  return c.json({ uploadId: id, partSize: p.partSize, partCount: p.partCount }, 201)
})

type Owned =
  | { ok: true; upload: Upload }
  | { ok: false; status: 403 | 404 | 409; code: string; message: string }

async function ownUpload(
  c: { env: Env; get: (k: 'device') => string },
  id: string,
): Promise<Owned> {
  const u = await c.env.DB.prepare('SELECT * FROM uploads WHERE id = ?1').bind(id).first<Upload>()
  if (!u)
    return {
      ok: false,
      status: 404,
      code: 'unknown-upload',
      message: 'There is no upload with that id.',
    }
  if (u.device !== c.get('device'))
    return {
      ok: false,
      status: 403,
      code: 'not-yours',
      message: 'This upload belongs to another device.',
    }
  if (u.state !== 'open')
    return {
      ok: false,
      status: 409,
      code: 'not-open',
      message: `This upload was already ${u.state}.`,
    }
  return { ok: true, upload: u }
}

app.post('/v1/uploads/:id/parts', async (c) => {
  const r = await ownUpload(c, c.req.param('id'))
  if (!r.ok) return fail(c, r.status, r.code, r.message)
  const u = r.upload
  const body = await c.req
    .json<{ from?: unknown; count?: unknown }>()
    .catch(() => ({}) as Record<string, unknown>)
  const from = body.from ?? 1
  const count = body.count ?? PRESIGN_BATCH
  if (typeof from !== 'number' || !Number.isInteger(from) || from < 1 || from > u.part_count)
    return fail(c, 400, 'bad-range', `\`from\` must be a part number from 1 to ${u.part_count}.`)
  if (typeof count !== 'number' || !Number.isInteger(count) || count < 1 || count > PRESIGN_BATCH)
    return fail(c, 400, 'bad-range', `\`count\` must be 1 to ${PRESIGN_BATCH}.`)
  const creds = credentials(c.env)
  if (!creds)
    return fail(
      c,
      503,
      'not-configured',
      "Uploads aren't set up on this server yet.",
      'Try again later.',
      300,
    )
  const last = Math.min(u.part_count, from + count - 1)
  const parts = []
  for (let n = from; n <= last; n++)
    parts.push({ n, url: await partUrl(creds, u.object_key, u.r2_upload_id, n) })
  return c.json({ parts })
})

app.post('/v1/uploads/:id/complete', async (c) => {
  const r = await ownUpload(c, c.req.param('id'))
  if (!r.ok) return fail(c, r.status, r.code, r.message)
  const u = r.upload
  const body = await c.req.json<{ parts?: unknown }>().catch(() => null)
  const parts = body?.parts
  if (!Array.isArray(parts) || parts.length !== u.part_count)
    return fail(
      c,
      400,
      'bad-parts',
      `\`parts\` must list all ${u.part_count} parts with their ETags.`,
    )
  const list: R2UploadedPart[] = []
  for (const [i, p] of parts.entries()) {
    const { n, etag } = (p ?? {}) as { n?: unknown; etag?: unknown }
    if (n !== i + 1 || typeof etag !== 'string' || etag.length === 0 || etag.length > 512)
      return fail(c, 400, 'bad-parts', `Part ${i + 1} is missing or out of order.`)
    list.push({ partNumber: n, etag })
  }
  const mpu = c.env.UPLOADS.resumeMultipartUpload(u.object_key, u.r2_upload_id)
  try {
    await mpu.complete(list)
  } catch (e) {
    return fail(
      c,
      409,
      'incomplete',
      "Storage couldn't put the parts together.",
      'Check every part was uploaded, then try again.',
    )
  }
  const slug = randomId()
  const now = Date.now()
  await c.env.DB.batch([
    c.env.DB.prepare("UPDATE uploads SET state = 'completed' WHERE id = ?1").bind(u.id),
    c.env.DB.prepare(
      'INSERT INTO shares (slug, upload_id, object_key, name, size, mime, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)',
    ).bind(
      slug,
      u.id,
      u.object_key,
      u.name,
      u.size,
      u.mime,
      now + u.expires_days * 86_400_000,
      now,
    ),
  ])
  return c.json({
    shareUrl: `${c.env.PUBLIC_URL}/s/${slug}`,
    expiresAt: now + u.expires_days * 86_400_000,
  })
})

type Share = {
  slug: string
  object_key: string
  name: string
  size: number
  expires_at: number
  max_downloads: number | null
  downloads: number
}

async function liveShare(
  env: Env,
  slug: string,
): Promise<{ share: Share } | { gone: string } | null> {
  if (!/^[0-9A-Za-z]{22}$/.test(slug)) return null
  const s = await env.DB.prepare('SELECT * FROM shares WHERE slug = ?1').bind(slug).first<Share>()
  if (!s) return null
  if (s.expires_at <= Date.now())
    return {
      gone: `This link expired on ${new Date(s.expires_at).toUTCString().slice(5, 16)}. Ask the sender for a new one.`,
    }
  if (s.max_downloads !== null && s.downloads >= s.max_downloads)
    return { gone: 'This link reached its download limit.' }
  return { share: s }
}

const esc = (s: string) => s.replace(/[&<>"']/g, (ch) => `&#${ch.charCodeAt(0)};`)

function page(title: string, body: string) {
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="robots" content="noindex"><title>${esc(title)}</title>
<style>body{font:16px/1.5 system-ui,sans-serif;margin:0;min-height:100dvh;display:grid;place-items:center;background:#0f1218;color:#e8eaf0}
main{max-width:30rem;padding:24px}h1{font-size:1.3rem;margin:0 0 4px;overflow-wrap:anywhere}p{color:#a3a9b8;margin:0 0 16px}
a.b{display:inline-block;background:#f08a3c;color:#16110c;padding:10px 18px;border-radius:8px;text-decoration:none;font-weight:600}</style>
</head><body><main>${body}</main></body></html>`
}

app.get('/s/:slug', async (c) => {
  const r = await liveShare(c.env, c.req.param('slug'))
  if (!r)
    return c.html(
      page(
        'Link not found',
        '<h1>Link not found</h1><p>Check the link, or ask the sender for it again.</p>',
      ),
      404,
    )
  if ('gone' in r)
    return c.html(page('Link expired', `<h1>This link is gone</h1><p>${esc(r.gone)}</p>`), 410)
  const s = r.share
  return c.html(
    page(
      s.name,
      `<h1>${esc(s.name)}</h1><p>${humanBytes(s.size)}, available until ${new Date(s.expires_at).toUTCString().slice(5, 16)}.</p>
       <a class="b" href="/s/${s.slug}/file">Download</a>`,
    ),
  )
})

app.get('/s/:slug/file', async (c) => {
  const r = await liveShare(c.env, c.req.param('slug'))
  if (!r) return fail(c, 404, 'unknown-link', 'Link not found.', 'Ask the sender for it again.')
  if ('gone' in r) return fail(c, 410, 'gone', r.gone)
  const creds = credentials(c.env)
  if (!creds)
    return fail(
      c,
      503,
      'not-configured',
      "Downloads aren't set up on this server yet.",
      'Try again later.',
      300,
    )
  await c.env.DB.prepare('UPDATE shares SET downloads = downloads + 1 WHERE slug = ?1')
    .bind(r.share.slug)
    .run()
  return c.redirect(await getUrl(creds, r.share.object_key, r.share.name), 302)
})

export default app
