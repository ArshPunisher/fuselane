import type { JobStatus } from '../lib/types'
import { mark, t } from '../lib/i18n'

export const STATUS_WORD: Record<JobStatus, string> = {
  queued: mark('Waiting'),
  running: mark('Downloading'),
  paused: mark('Paused'),
  failed: mark('Stopped'),
  'failed-final': mark('Failed'),
  completed: mark('Done'),
  cancelled: mark('Cancelled'),
}

/** A download's state in a word, in the language in use. */
export function statusWord(s: JobStatus): string {
  return t(STATUS_WORD[s])
}
