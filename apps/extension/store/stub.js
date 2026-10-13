// Stand-in browser API so the built popup renders outside an extension (store screenshots only).
globalThis.chrome = globalThis.browser = {
  runtime: {
    id: 'store-screenshot',
    getManifest: () => ({ version: '0.4.1' }),
    sendMessage: async () => null,
    onMessage: { addListener: () => {} },
    sendNativeMessage: async () => ({ type: 'pong', app: { version: '0.1.0' } }),
  },
  storage: {
    local: { get: async () => ({}), set: async () => {} },
    onChanged: { addListener: () => {} },
  },
  // A tab on a downloads page, for the "On this page" list (0.2.0).
  tabs: {
    query: async () => [
      { id: 1, url: 'https://releases.example.org/26.04/', title: 'Ubuntu 26.04 LTS releases' },
    ],
  },
  scripting: {
    executeScript: async () => [
      {
        result: [
          { url: 'ubuntu-26.04-desktop-amd64.iso', kind: 'link', label: 'Desktop image' },
          { url: 'ubuntu-26.04-live-server-amd64.iso', kind: 'link', label: 'Server image' },
          { url: 'release-notes.pdf', kind: 'link', label: 'Release notes' },
        ],
      },
    ],
  },
  downloads: { download: async () => 1 },
  permissions: { contains: async () => true },
}
