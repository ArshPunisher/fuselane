// Stand-in browser API so the built popup renders outside an extension (store screenshots only).
globalThis.chrome = globalThis.browser = {
  runtime: {
    id: 'store-screenshot',
    sendNativeMessage: async () => ({ type: 'pong', app: { version: '0.1.0' } }),
  },
  storage: { local: { get: async () => ({}), set: async () => {} } },
  // A tab on a downloads page, for the "On this page" list (0.2.0).
  tabs: {
    query: async () => [{ id: 1, url: 'https://releases.example.org/26.04/' }],
  },
  scripting: {
    executeScript: async () => [
      {
        result: [
          { url: 'ubuntu-26.04-desktop-amd64.iso', kind: 'link', label: 'Desktop image' },
          { url: 'ubuntu-26.04-live-server-amd64.iso', kind: 'link', label: 'Server image' },
          { url: 'intro-video.mp4', kind: 'video', label: 'Tour' },
          { url: 'release-notes.pdf', kind: 'link', label: '' },
        ],
      },
    ],
  },
  downloads: { download: async () => 1 },
}
