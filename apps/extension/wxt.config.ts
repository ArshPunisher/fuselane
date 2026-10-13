import { defineConfig } from 'wxt'

/**
 * The Chrome Web Store listing's public key. Only a local build carries it
 * (`pnpm build:local`): an unpacked copy then has the store's id, so the
 * installed Fuselane app (which only answers the store ids) talks to it. Store
 * uploads never include it.
 */
const STORE_KEY =
  'MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAwk9X9z2865KeAiUvxX1Gy6iU5QKZdMmo6SGNxAzlxMcRBHgcgpj4MpgEv1j2scEQDIyvEEOa8aThi/Oqz1ucVr3TZqhNz2mMJBhDIgJ8fM5BwJ9xdYkwKCoxxbVT53/BtdGsIUZIxFYwr4fvy+FVHjwUMnjzx7wlhheA9FuZovpbs2fO0D6iY9Xdp9pHKLEiUwqtntimjUvzq6avHozxbrMGCHPDmF+RWkj2LSrK/78/BeCcVz2ePjbxGb4Wz52iKvXSloLkXQcWyrGmlNHg/gbkq/o0zOuoD2yZQmqLvWgQie7kyMuRwcp8CNPF4W/xrB+GgRyGTq+x6f6vrnVOlQIDAQAB'
const LOCAL = process.env.FUSELANE_LOCAL === '1'

// Fuselane's browser extension (BROWSER-EXTENSION.md, REVIEW-2026-10-11 D): hands
// downloads to the app through native messaging, or downloads them itself up to
// a limit when the app isn't installed.
export default defineConfig({
  // Test and local builds go to their own folders so they can never be zipped
  // for a store.
  ...(process.env.FUSELANE_E2E === '1' ? { outDir: '.output-e2e' } : {}),
  ...(LOCAL ? { outDir: '.output-local' } : {}),
  hooks: {
    // The link-click script is registered at run time, only after the person
    // allows Fuselane on all sites (optional_host_permissions). WXT would add
    // its matches as a required host permission, which means an install-time
    // warning and current users switched off: keep it optional.
    'build:manifestGenerated': (_wxt, manifest) => {
      delete manifest.host_permissions
      // Test builds only (e2e/self.spec.ts): a browser under test can't click
      // the permission prompt, so site access is granted at load instead.
      if (process.env.FUSELANE_E2E === '1') manifest.host_permissions = ['<all_urls>']
      // Chrome and Edge only: Firefox has its own fixed id.
      if (LOCAL && !manifest.browser_specific_settings) manifest.key = STORE_KEY
    },
  },
  manifest: ({ browser }) => ({
    name: 'Fuselane',
    description: 'Download faster with Fuselane: it splits each download across all your networks.',
    homepage_url: 'https://fuselane.app',
    // activeTab + scripting: the popup lists the current tab's file links and
    // feeds, only when opened (no access to other tabs, nothing in the background).
    // offscreen: the extension's own downloads run in an offscreen page (the
    // service worker can stop at any time). downloads.ui: hides the browser's
    // download bar for downloads Fuselane takes. Neither exists in Firefox, and
    // neither adds an install warning.
    permissions: [
      'downloads',
      'nativeMessaging',
      'storage',
      'contextMenus',
      'activeTab',
      'scripting',
      ...(browser === 'firefox' ? [] : ['offscreen', 'downloads.ui']),
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
