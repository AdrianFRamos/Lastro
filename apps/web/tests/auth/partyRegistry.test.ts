import { address, getAddressEncoder, getProgramDerivedAddress } from '@solana/kit'
import { describe, expect, it } from 'vitest'
import {
  PARTY_RECORD_SIZE,
  PARTY_ROLE,
  PARTY_STATUS,
  decodeParty,
  findPartiesByWallet,
  workspaceRoleOf,
  type PartyAccount,
} from '../../src/auth/partyRegistry'
import { webConfig } from '../../src/config'

const WALLET = 'Vote111111111111111111111111111111111111111'
const OTHER_WALLET = 'Config1111111111111111111111111111111111111'
const PARTY_ID = new Uint8Array(32).fill(7)

function hexBytes(hex: string): Uint8Array {
  return Uint8Array.from(hex.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16))
}

async function partyAccount(
  options: { wallet?: string; role?: number; status?: number; deploymentId?: Uint8Array } = {},
): Promise<PartyAccount> {
  const discriminator = new Uint8Array(
    await crypto.subtle.digest('SHA-256', new TextEncoder().encode('account:PartyRecord')),
  ).subarray(0, 8)
  const [pda, bump] = await getProgramDerivedAddress({
    programAddress: address(webConfig.lastroProgramId),
    seeds: [
      new TextEncoder().encode('party'),
      options.deploymentId ?? hexBytes(webConfig.lastroDeploymentId),
      PARTY_ID,
    ],
  })
  const data = new Uint8Array(PARTY_RECORD_SIZE)
  data.set(discriminator, 0)
  data.set(PARTY_ID, 8)
  data.set(getAddressEncoder().encode(address(options.wallet ?? WALLET)), 40)
  new DataView(data.buffer).setUint16(72, options.role ?? 6, true)
  data[74] = options.status ?? PARTY_STATUS.ACTIVE
  data[75] = bump
  return { address: String(pda), data }
}

describe('auth/partyRegistry on-chain party lookup', () => {
  /**
   * ARRANGE: build a PartyRecord exactly as the program stores it for this deployment.
   * ACTION: decode it for its own wallet.
   * ASSERT: id, wallet, role and status come back as stored.
   * FAILURE MEANS: a registered participant could never sign in with its wallet.
   */
  it('decodes a party of this deployment registered for the wallet', async () => {
    const party = await decodeParty(await partyAccount(), WALLET)

    expect(party).toMatchObject({
      partyIdHex: '07'.repeat(32),
      wallet: WALLET,
      role: 6,
      status: PARTY_STATUS.ACTIVE,
    })
  })

  /**
   * ARRANGE: valid-looking PartyRecords that belong to another deployment or another wallet,
   *          and one whose discriminator or length was altered.
   * ACTION: decode each one for WALLET.
   * ASSERT: every one is rejected.
   * FAILURE MEANS: a lying RPC or a party from another deployment could grant a workspace role.
   */
  it('rejects parties of other deployments, other wallets or forged layouts', async () => {
    const foreignDeployment = await partyAccount({ deploymentId: new Uint8Array(32).fill(9) })
    const otherWallet = await partyAccount({ wallet: OTHER_WALLET })
    const forged = await partyAccount()
    forged.data[0] = forged.data[0]! ^ 0xff
    const truncated = await partyAccount()

    expect(await decodeParty(foreignDeployment, WALLET)).toBeNull()
    expect(await decodeParty(otherWallet, WALLET)).toBeNull()
    expect(await decodeParty(forged, WALLET)).toBeNull()
    expect(
      await decodeParty({ ...truncated, data: truncated.data.subarray(0, 70) }, WALLET),
    ).toBeNull()
  })

  /**
   * ARRANGE: an RPC fetcher that returns one valid party and one foreign party.
   * ACTION: look up the wallet's parties.
   * ASSERT: only the verified party is kept.
   * FAILURE MEANS: the login would trust whatever accounts the RPC chose to return.
   */
  it('keeps only locally verified parties from the RPC answer', async () => {
    const valid = await partyAccount()
    const foreign = await partyAccount({ deploymentId: new Uint8Array(32).fill(9) })

    const parties = await findPartiesByWallet(WALLET, async () => [valid, foreign])

    expect(parties.map((party) => party.address)).toEqual([valid.address])
  })

  /**
   * ARRANGE: every on-chain party role code.
   * ACTION: map each to a workspace profile.
   * ASSERT: chain participants get their own workspace; every other role is read-only.
   * FAILURE MEANS: a wallet could open a workspace its on-chain role does not grant.
   */
  it('maps on-chain roles to workspace profiles', () => {
    expect(
      Object.fromEntries(
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 99].map((r) => [r, workspaceRoleOf(r)]),
      ),
    ).toEqual({
      1: 'producer',
      2: 'viewer',
      3: 'viewer',
      4: 'viewer',
      5: 'carrier',
      6: 'slaughterhouse',
      7: 'slaughterhouse',
      8: 'viewer',
      9: 'merchant',
      10: 'viewer',
      11: 'viewer',
      12: 'exporter',
      99: 'viewer',
    })
  })

  /**
   * ARRANGE: the dedicated exporter role and the retailer role.
   * ACTION: map both to workspace profiles.
   * ASSERT: the exporter (transport out of the country) and the retailer (commerce) open
   *         different workspaces, and the exporter has the same records as the carrier.
   * FAILURE MEANS: an exporter wallet could land in the commerce workspace, or vice versa.
   */
  it('keeps the exporter distinct from commerce and shaped like transport', async () => {
    const { resourcesOf } = await import('../../src/demo/roles')
    expect(workspaceRoleOf(PARTY_ROLE.EXPORTER)).toBe('exporter')
    expect(workspaceRoleOf(PARTY_ROLE.RETAILER)).toBe('merchant')
    expect(resourcesOf('exporter').map((r) => r.label.pt)).toEqual(
      resourcesOf('carrier').map((r) => r.label.pt),
    )
    expect(resourcesOf('merchant').map((r) => r.label.pt)).not.toEqual(
      resourcesOf('exporter').map((r) => r.label.pt),
    )
  })
})
