import { defineConfig } from 'wxt'

// Fuselane's browser extension (BROWSER-EXTENSION.md): a thin capturer that hands
// big downloads to the app through native messaging.
export default defineConfig({
  manifest: ({ browser }) => ({
    name: 'Fuselane',
    description: 'Hands big downloads to Fuselane, which splits them across all your networks.',
    // No host permissions: cookies aren't forwarded yet, so none are read.
    permissions: ['downloads', 'nativeMessaging', 'storage', 'contextMenus'],
    // Firefox needs a fixed id for native messaging's allowed_extensions.
    ...(browser === 'firefox'
      ? {
          browser_specific_settings: {
            gecko: { id: 'fuselane@fuselane.app', strict_min_version: '128.0' },
          },
        }
      : {}),
  }),
})
