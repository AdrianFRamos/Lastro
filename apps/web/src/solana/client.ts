import { createClient } from '@solana/kit'
import { solanaRpc } from '@solana/kit-plugin-rpc'
import { walletSigner } from '@solana/kit-plugin-wallet'
import { webConfig } from '../config'

/**
 * Single browser Solana client.
 *
 * Wallet Standard keeps private-key material inside the wallet. Lastro uses the connected
 * wallet as both fee payer and identity signer for hackathon transactions, matching the
 * `walletSigner` plugin semantics. The chain and RPC endpoint come only from explicit build
 * configuration so the UI cannot silently sign on a different cluster.
 */
export const solanaClient = createClient()
  .use(walletSigner({ chain: webConfig.solanaChain }))
  .use(solanaRpc({ rpcUrl: webConfig.solanaRpcUrl }))
