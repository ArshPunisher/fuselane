// Stand-in browser API so the built popup renders outside an extension (store screenshots only).
globalThis.chrome = globalThis.browser = {
  runtime: {
    id: 'store-screenshot',
    sendNativeMessage: async () => ({ type: 'pong', app: { version: '0.1.0' } }),
  },
  storage: { local: { get: async () => ({}), set: async () => {} } },
}
