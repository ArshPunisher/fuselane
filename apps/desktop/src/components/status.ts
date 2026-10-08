import type { JobStatus } from '../lib/types'

export const STATUS_WORD: Record<JobStatus, string> = {
  queued: 'Waiting',
  running: 'Downloading',
  paused: 'Paused',
  failed: 'Stopped',
  'failed-final': 'Failed',
  completed: 'Done',
  cancelled: 'Cancelled',
}
