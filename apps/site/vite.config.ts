import { defineConfig } from 'vite'

// The download page, served by GitHub Pages under /fuselane/.
export default defineConfig({
  base: './',
  build: { target: 'es2022' },
  server: { port: 5195, strictPort: true, fs: { allow: ['../..'] } },
})
