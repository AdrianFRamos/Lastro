/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_BASE_URL: string
  readonly VITE_SOLANA_RPC_URL: string
  readonly VITE_SOLANA_CHAIN: string
  readonly VITE_LASTRO_PROGRAM_ID: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
