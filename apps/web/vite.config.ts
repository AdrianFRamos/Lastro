import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

/**
 * The repository keeps one root .env for API/Agent/Web orchestration. Vite loads from
 * envDir but still exposes only variables matching its VITE_ prefix to browser code.
 * ../../ resolves from apps/web to repository root. This avoids duplicating deployment
 * identity across multiple environment files.
 */
export default defineConfig({
  envDir: fileURLToPath(new URL('../../', import.meta.url)),
  plugins: [
    vue({
      template: {
        transformAssetUrls: {
          includeAbsolute: false,
        },
      },
    }),
  ],
  test: {
    environment: 'jsdom',
    environmentOptions: {
      jsdom: {
        url: 'http://localhost:3000/',
      },
    },
    setupFiles: ['./tests/setup.ts'],
    include: ['tests/**/*.test.ts'],
    coverage: { reporter: ['text', 'lcov'] },
  },
})
