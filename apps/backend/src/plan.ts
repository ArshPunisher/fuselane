// How a file is cut into parts for R2 multipart upload (BONDED-UPLOADS.md §2).
// R2 needs every part but the last to be the same size, at least 5 MiB, and at
// most 10,000 parts, so the size is chosen once and never changes mid-upload.

export const MIB = 1024 * 1024
export const MIN_PART = 16 * MIB
export const MAX_PARTS = 10_000
/** The free tier's largest link (BONDED-UPLOADS.md §6). */
export const MAX_SIZE = 50 * 1024 * MIB

export interface Plan {
  partSize: number
  partCount: number
}

/** max(16 MiB, ceil(size / 9,000)), rounded up to a whole MiB. */
export function plan(size: number): Plan {
  const wanted = Math.max(MIN_PART, Math.ceil(size / 9_000))
  const partSize = Math.ceil(wanted / MIB) * MIB
  return { partSize, partCount: Math.max(1, Math.ceil(size / partSize)) }
}
