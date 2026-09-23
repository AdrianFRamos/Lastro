import { defineConfig, devices } from '@playwright/test'
import process from 'node:process'

const externalWeb = process.env.LASTRO_E2E_EXTERNAL === '1'

const inheritedEnvironment = Object.fromEntries(
  Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined),
)

const webServerEnv: Record<string, string> = {
  ...inheritedEnvironment,
  VITE_API_BASE_URL:
    process.env.VITE_API_BASE_URL ?? process.env.LASTRO_SYSTEM_API_URL ?? 'http://127.0.0.1:8080',
  VITE_SOLANA_RPC_URL:
    process.env.VITE_SOLANA_RPC_URL ??
    process.env.LASTRO_SYSTEM_SOLANA_RPC_URL ??
    'http://127.0.0.1:8899',
  VITE_SOLANA_CHAIN:
    process.env.VITE_SOLANA_CHAIN ?? process.env.LASTRO_E2E_SOLANA_CHAIN ?? 'solana:localnet',
  VITE_LASTRO_PROGRAM_ID:
    process.env.VITE_LASTRO_PROGRAM_ID ??
    process.env.LASTRO_PROGRAM_ID ??
    '11111111111111111111111111111111',
}

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  workers: 1,
  timeout: 120_000,
  expect: { timeout: 15_000 },
  retries: 0,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: process.env.LASTRO_E2E_BASE_URL ?? 'http://127.0.0.1:4173',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: externalWeb
    ? undefined
    : {
        command: 'npm run build && npm run preview -- --host 127.0.0.1 --port 4173',
        port: 4173,
        reuseExistingServer: !process.env.CI,
        env: webServerEnv,
      },
})
