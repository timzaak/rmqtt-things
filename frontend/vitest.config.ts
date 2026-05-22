import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'
import { fileURLToPath } from 'url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

export default defineConfig({
  plugins: [
    tailwindcss(),
    tanstackRouter({
      target: 'react',
      autoCodeSplitting: true,
      routeFileIgnorePattern: '__tests__',
    }),
    react(),
  ],
  resolve: {
    alias: { '@': path.resolve(__dirname, './src') },
  },
  optimizeDeps: {
    include: ['class-variance-authority', 'clsx', 'tailwind-merge', 'react', 'react-dom'],
  },
  test: {
    environment: 'jsdom',
    isolate: true,
    globals: true,
    reporters: ['minimal'],
    testTimeout: 5000,
    include: ['**/__tests__/**/*.{test,spec}.{js,jsx,ts,tsx}'],
    exclude: ['**/node_modules/**', '**/dist/**', '**/.git/**', '**/.vscode/**'],
    setupFiles: ['./src/test/setup.ts'],
  },
})
