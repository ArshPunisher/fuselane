'use client'

import { useEffect } from 'react'
import { usePathname } from 'next/navigation'
import { initShell } from '@/lib/legacy/common'
import { NavMarkup } from './nav-markup'

let started = false

/** The navbar: wired once per visit, told about every page change. */
export function Nav() {
  const pathname = usePathname()
  useEffect(() => {
    if (started) return
    started = true
    initShell()
  }, [])
  useEffect(() => {
    window.dispatchEvent(new Event('fuselane:route'))
  }, [pathname])
  return <NavMarkup current={pathname} />
}
