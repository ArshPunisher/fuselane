// The extension's own downloads (no app installed): what the popup lists and
// what survives the browser closing (REVIEW-2026-10-11 D). Kept in
// storage.local under one key; the runner is the only writer while one runs.
import type { Part } from './fetcher.ts'

export type SelfStatus =
  'queued' | 'running' | 'paused' | 'saving' | 'completed' | 'failed' | 'too-big'

export interface SelfJob {
  id: string
  url: string
  /** Where redirects ended; parts are fetched from here. */
  finalUrl: string
  pageUrl: string | null
  name: string
  total: number | null
  written: number
  /** Bytes per second right now (0 when not running). */
  rate: number
  status: SelfStatus
  parts: Part[] | null
  error: string | null
  /** The browser's download id once saved, to open or show it. */
  savedId: number | null
  createdAt: number
}

export const KEY = 'selfJobs'
/** Finished ones kept in the list. */
const KEEP_DONE = 30

export function readJobs(stored: unknown): SelfJob[] {
  return Array.isArray(stored)
    ? (stored.filter((j) => j && typeof j.id === 'string') as SelfJob[])
    : []
}

/** Newest active first, then the latest finished ones, at most KEEP_DONE of those. */
export function tidy(jobs: SelfJob[]): SelfJob[] {
  const active = (j: SelfJob) => ['queued', 'running', 'paused', 'saving'].includes(j.status)
  const live = jobs.filter(active).sort((a, b) => b.createdAt - a.createdAt)
  const done = jobs
    .filter((j) => !active(j))
    .sort((a, b) => b.createdAt - a.createdAt)
    .slice(0, KEEP_DONE)
  return [...live, ...done]
}

export function newJob(
  url: string,
  pageUrl: string | null,
  name: string,
  total: number | null,
): SelfJob {
  return {
    id: `x${Date.now().toString(36)}${Math.random().toString(36).slice(2, 7)}`,
    url,
    finalUrl: url,
    pageUrl,
    name,
    total,
    written: 0,
    rate: 0,
    status: 'queued',
    parts: null,
    error: null,
    savedId: null,
    createdAt: Date.now(),
  }
}
