import type { JobStatus, JobView } from '../lib/types'
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

/** Where a video with separate sound is, once its video part is done (B5). */
export function soundWord(job: Pick<JobView, 'sound'>): string | null {
  if (job.sound === 'getting') return t('Getting the sound…')
  if (job.sound === 'joining') return t('Joining video and sound…')
  if (job.sound === 'missing') return t('No sound yet')
  return null
}
