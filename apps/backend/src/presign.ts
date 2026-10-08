// Presigned R2 (S3 API) URLs: the app PUTs parts straight to R2 over each network,
// and receivers GET the file from R2, with no bytes through the Worker and no R2
// credentials on the client (BONDED-UPLOADS.md §3).
import { AwsClient } from 'aws4fetch'

export interface R2Credentials {
  accountId: string
  accessKeyId: string
  secretAccessKey: string
  bucket: string
}

export function credentials(env: Record<string, unknown>): R2Credentials | null {
  const { R2_ACCOUNT_ID, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY, R2_BUCKET } = env as Record<
    string,
    string | undefined
  >
  if (!R2_ACCOUNT_ID || !R2_ACCESS_KEY_ID || !R2_SECRET_ACCESS_KEY || !R2_BUCKET) return null
  return {
    accountId: R2_ACCOUNT_ID,
    accessKeyId: R2_ACCESS_KEY_ID,
    secretAccessKey: R2_SECRET_ACCESS_KEY,
    bucket: R2_BUCKET,
  }
}

function objectUrl(c: R2Credentials, key: string): URL {
  const path = key.split('/').map(encodeURIComponent).join('/')
  return new URL(`https://${c.accountId}.r2.cloudflarestorage.com/${c.bucket}/${path}`)
}

async function sign(
  c: R2Credentials,
  url: URL,
  method: string,
  expiresSeconds: number,
): Promise<string> {
  url.searchParams.set('X-Amz-Expires', String(expiresSeconds))
  const client = new AwsClient({
    accessKeyId: c.accessKeyId,
    secretAccessKey: c.secretAccessKey,
    service: 's3',
    region: 'auto',
  })
  const signed = await client.sign(new Request(url, { method }), { aws: { signQuery: true } })
  return signed.url
}

/** A URL to PUT one part, valid for `expiresSeconds`. */
export function partUrl(
  c: R2Credentials,
  key: string,
  uploadId: string,
  partNumber: number,
  expiresSeconds = 3600,
) {
  const url = objectUrl(c, key)
  url.searchParams.set('partNumber', String(partNumber))
  url.searchParams.set('uploadId', uploadId)
  return sign(c, url, 'PUT', expiresSeconds)
}

/** A URL to GET the whole object (ranges work), valid for `expiresSeconds`. */
export function getUrl(c: R2Credentials, key: string, filename: string, expiresSeconds = 600) {
  const url = objectUrl(c, key)
  url.searchParams.set(
    'response-content-disposition',
    `attachment; filename*=UTF-8''${encodeURIComponent(filename)}`,
  )
  return sign(c, url, 'GET', expiresSeconds)
}
