import type { NextConfig } from 'next'

// Static export for Cloudflare Pages (fuselane.app). The page scripts are the
// plain site's imperative code, so strict mode's double effects are off.
const config: NextConfig = {
  output: 'export',
  trailingSlash: true,
  images: { unoptimized: true },
  reactStrictMode: false,
}

export default config
