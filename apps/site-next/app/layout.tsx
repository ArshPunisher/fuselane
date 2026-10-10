import type { Metadata, Viewport } from 'next'
import '@/styles/base.css'
import '@/styles/home.css'
import '@/styles/pages.css'
import '@/styles/receive.css'
import { Nav } from '@/components/shell'
import { FooterMarkup } from '@/components/footer-markup'

export const metadata: Metadata = {
  metadataBase: new URL('https://fuselane.app'),
  icons: {
    icon: [
      { url: '/favicon.ico', sizes: '32x32' },
      { url: '/favicon.svg', type: 'image/svg+xml' },
    ],
    apple: '/apple-touch-icon.png',
  },
  manifest: '/site.webmanifest',
}

export const viewport: Viewport = {
  width: 'device-width',
  initialScale: 1,
  colorScheme: 'light dark',
  themeColor: [
    { media: '(prefers-color-scheme: dark)', color: '#0a0c11' },
    { media: '(prefers-color-scheme: light)', color: '#f9fafc' },
  ],
}

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <Nav />
        {children}
        <FooterMarkup />
      </body>
    </html>
  )
}
