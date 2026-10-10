import { defineConfig } from 'wxt'

// Fuselane's browser extension (BROWSER-EXTENSION.md): a thin capturer that hands
// big downloads to the app through native messaging.
export default defineConfig({
  manifest: ({ browser }) => ({
    name: 'Fuselane',
    description: 'Hands big downloads to Fuselane, which splits them across all your networks.',
    homepage_url: 'https://fuselane.app',
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
          // Who makes it, shown on Firefox's add-on page.
          developer: { name: 'ClearCraft Labs', url: 'https://clearcraftlabs.com' },
          browser_specific_settings: {
            gecko: {
              id: 'fuselane@fuselane.app',
              // 140 is the first Firefox that reads data_collection_permissions.
              strict_min_version: '140.0',
              // Required for new listings on addons.mozilla.org: Fuselane's
              // extension collects and sends nothing to anyone.
              data_collection_permissions: { required: ['none'] },
            },
          },
        }
      : {}),
  }),
})
