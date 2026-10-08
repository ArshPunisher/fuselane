import { describe, expect, it } from 'vitest'
import { credentials, getUrl, partUrl } from '../src/presign'

// Fake credentials: only the URL's shape is checked here; spike S6 checks real R2.
const creds = {
  accountId: 'acc123',
  accessKeyId: 'AKIDEXAMPLE',
  secretAccessKey: 'secret',
  bucket: 'fuselane-uploads',
}

describe('presigned URLs', () => {
  it('a part URL targets the bucket, part and upload, signed for an hour', async () => {
    const u = new URL(await partUrl(creds, 'u/abc/My File.bin', 'up-1', 7))
    expect(u.host).toBe('acc123.r2.cloudflarestorage.com')
    expect(u.pathname).toBe('/fuselane-uploads/u/abc/My%20File.bin')
    expect(u.searchParams.get('partNumber')).toBe('7')
    expect(u.searchParams.get('uploadId')).toBe('up-1')
    expect(u.searchParams.get('X-Amz-Expires')).toBe('3600')
    expect(u.searchParams.get('X-Amz-Signature')).toMatch(/^[0-9a-f]{64}$/)
    expect(u.searchParams.get('X-Amz-Credential')).toContain('AKIDEXAMPLE/')
    expect(u.toString()).not.toContain('secret')
  })

  it('a download URL is short-lived and names the file for the browser', async () => {
    const u = new URL(await getUrl(creds, 'u/abc/naïve.txt', 'naïve.txt'))
    expect(u.searchParams.get('X-Amz-Expires')).toBe('600')
    expect(u.searchParams.get('response-content-disposition')).toBe(
      "attachment; filename*=UTF-8''na%C3%AFve.txt",
    )
  })

  it('missing secrets mean no credentials, never a half-configured signer', () => {
    expect(credentials({ R2_BUCKET: 'b' })).toBeNull()
    expect(
      credentials({
        R2_ACCOUNT_ID: 'a',
        R2_ACCESS_KEY_ID: 'k',
        R2_SECRET_ACCESS_KEY: 's',
        R2_BUCKET: 'b',
      }),
    ).not.toBeNull()
  })
})
