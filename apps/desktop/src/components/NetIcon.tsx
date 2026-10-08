import {
  CellSignalFull,
  DeviceMobile,
  Globe,
  PlugsConnected,
  ShieldCheck,
  WifiHigh,
} from '@phosphor-icons/react'

export function NetIcon({ kind, size = 18 }: { kind: string; size?: number }) {
  const p = { size, 'aria-hidden': true } as const
  switch (kind) {
    case 'wifi':
      return <WifiHigh {...p} />
    case 'tether':
      return <DeviceMobile {...p} />
    case 'ethernet':
      return <PlugsConnected {...p} />
    case 'cellular':
      return <CellSignalFull {...p} />
    case 'vpn':
      return <ShieldCheck {...p} />
    default:
      return <Globe {...p} />
  }
}
