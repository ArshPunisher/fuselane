import { resolve } from 'node:path'
import { defineConfig } from 'vite'

// The site, served by GitHub Pages under /fuselane/: home, download, FAQ, support.
export default defineConfig({
  base: './',
  build: {
    target: 'es2022',
    rollupOptions: {
      input: {
        home: resolve(import.meta.dirname, 'index.html'),
        download: resolve(import.meta.dirname, 'download/index.html'),
        faq: resolve(import.meta.dirname, 'faq/index.html'),
        support: resolve(import.meta.dirname, 'support/index.html'),
        send: resolve(import.meta.dirname, 's/index.html'),
      },
    },
  },
  server: { port: 5195, strictPort: true, fs: { allow: ['../..'] } },
})
