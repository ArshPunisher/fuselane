import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Tauri serves the built files; `pnpm dev` runs the same UI in a browser with demo data.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5190, strictPort: true },
  build: { target: 'safari16', sourcemap: false },
})
