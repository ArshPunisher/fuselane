'use client'

import { useEffect } from 'react'
import { initPage } from '@/lib/legacy/common'
import { setPageSignal } from '@/lib/legacy/motion'

type Name = 'home' | 'download' | 'support' | 'receive'

// The page scripts from the plain site, loaded only on their own page.
const SCRIPTS: Record<Name, () => Promise<() => void>> = {
  home: () => import('@/lib/legacy/home').then((m) => m.initHome),
  download: () => import('@/lib/legacy/download').then((m) => m.initDownload),
  support: () => import('@/lib/legacy/support').then((m) => m.initSupport),
  receive: () => import('@/lib/legacy/receive').then((m) => m.initReceive),
}

/**
 * Runs the shared page behaviour (reveals, spotlights, copy buttons, the
 * star count, topic lists) and this page's own script, then stops all of it
 * when the visitor moves to another page.
 */
export function PageScript({ name }: { name?: Name }) {
  useEffect(() => {
    const ac = new AbortController()
    setPageSignal(ac.signal)
    initPage()
    if (name) {
      SCRIPTS[name]().then((init) => {
        if (ac.signal.aborted) return
        setPageSignal(ac.signal)
        init()
      })
    }
    return () => {
      ac.abort()
      setPageSignal(undefined)
    }
  }, [name])
  return null
}
