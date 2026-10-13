import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1421,
    strictPort: true,
    watch: {
      ignored: [
        '**/src-tauri/target/**',
        '**/src-tauri/native/capture-helper/.build/**',
      ],
    },
  },
  envPrefix: ['VITE_', 'TAURI_'],
  test: {
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    css: true,
    include: ['src/**/*.test.{ts,tsx}'],
  },
})
