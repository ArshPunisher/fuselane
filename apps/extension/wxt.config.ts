import { defineConfig } from 'wxt'

// Fuselane's browser extension (BROWSER-EXTENSION.md): a thin capturer that hands
// big downloads to the app through native messaging.
export default defineConfig({
  manifest: ({ browser }) => ({
    name: 'Fuselane',
    description: 'Hands big downloads to Fuselane, which splits them across all your networks.',
    // activeTab + scripting: the popup lists the current tab's videos and file
    // links, only when opened (no access to other tabs, nothing in the background).
    permissions: [
      'downloads',
      'nativeMessaging',
      'storage',
      'contextMenus',
      'activeTab',
      'scripting',
    ],
    // Signed-in downloads: asked for only when the person turns them on in settings.
    optional_permissions: ['cookies'],
    optional_host_permissions: ['<all_urls>'],
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
