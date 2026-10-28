import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    // Send API calls to the Rust server so the session cookie stays on the same site.
    // 127.0.0.1 rather than localhost because the server only listens on IPv4.
    proxy: {
      '/api': 'http://127.0.0.1:3000',
    },
  },
})
