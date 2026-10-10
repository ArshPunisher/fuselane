import type { MetadataRoute } from 'next'

export const dynamic = 'force-static'

// The pages search engines should list, as on the plain site.
const LISTED: [string, MetadataRoute.Sitemap[number]['changeFrequency'], number][] = [
  ['/', 'weekly', 1.0],
  ['/download/', 'weekly', 0.9],
  ['/guide/', 'monthly', 0.8],
  ['/faq/', 'monthly', 0.7],
  ['/idm-alternative/', 'monthly', 0.8],
  ['/combine-internet/', 'monthly', 0.8],
  ['/send-large-files/', 'monthly', 0.8],
  ['/support/', 'monthly', 0.5],
  ['/privacy/', 'yearly', 0.3],
  ['/terms/', 'yearly', 0.3],
]

export default function sitemap(): MetadataRoute.Sitemap {
  const lastModified = new Date()
  return LISTED.map(([path, changeFrequency, priority]) => ({
    url: `https://fuselane.app${path}`,
    lastModified,
    changeFrequency,
    priority,
  }))
}
