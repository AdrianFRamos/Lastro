import { ed25519 } from '@noble/curves/ed25519.js'
import { getAddressDecoder } from '@solana/kit'
import { describe, expect, it } from 'vitest'
import { PARTY_STATUS, type Party } from '../../src/auth/partyRegistry'
import {
  buildLoginMessage,
  chooseSession,
  loginWithConnectedWallet,
  verifyLoginSignature,
} from '../../src/auth/walletLogin'
import { webConfig } from '../../src/config'

function walletKey() {
  const secretKey = ed25519.utils.randomSecretKey()
  const wallet = getAddressDecoder().decode(ed25519.getPublicKey(secretKey))
  return { secretKey, wallet }
}

function party(role: number, status: number = PARTY_STATUS.ACTIVE, id = '07'): Party {
  return { address: 'x', partyIdHex: id.repeat(32), wallet: 'w', role, status }
}

describe('auth/walletLogin', () => {
  /**
   * ARRANGE: fixed login fields.
   * ACTION: build the login message.
   * ASSERT: it names the site, wallet, deployment, program, nonce and time, one per line.
   * FAILURE MEANS: a signature for one site, deployment or moment could be reused for another.
   */
  it('binds the login message to the site, wallet, deployment and a fresh nonce', () => {
    expect(
      buildLoginMessage({
        domain: 'lastro.dev.br',
        wallet: 'W',
        nonce: 'n1',
        issuedAt: '2026-10-11T00:00:00.000Z',
      }),
    ).toBe(
      [
        'Lastro login v1',
        'domain=lastro.dev.br',
        'wallet=W',
        `deploymentId=${webConfig.lastroDeploymentId}`,
        `programId=${webConfig.lastroProgramId}`,
        'nonce=n1',
        'issuedAt=2026-10-11T00:00:00.000Z',
        '',
      ].join('\n'),
    )
  })

  /**
   * ARRANGE: a wallet key and a login message it signed.
   * ACTION: verify the genuine signature, a tampered message and another wallet's signature.
   * ASSERT: only the genuine signature by the claimed wallet is accepted.
   * FAILURE MEANS: anyone could sign in as a registered wallet without holding its key.
   */
  it('accepts only a signature made by the claimed wallet over the exact message', () => {
    const owner = walletKey()
    const intruder = walletKey()
    const message = new TextEncoder().encode('Lastro login v1\nnonce=abc\n')
    const signature = ed25519.sign(message, owner.secretKey)

    expect(verifyLoginSignature(message, signature, owner.wallet)).toBe(true)
    expect(
      verifyLoginSignature(
        new TextEncoder().encode('Lastro login v1\nnonce=xyz\n'),
        signature,
        owner.wallet,
      ),
    ).toBe(false)
    expect(
      verifyLoginSignature(message, ed25519.sign(message, intruder.secretKey), owner.wallet),
    ).toBe(false)
    expect(verifyLoginSignature(message, signature.subarray(0, 63), owner.wallet)).toBe(false)
  })

  /**
   * ARRANGE: party lists a wallet can have on-chain.
   * ACTION: choose the workspace.
   * ASSERT: an active participant role opens its workspace; a non-participant or unregistered
   *         wallet is read-only; inactive registrations block the login.
   * FAILURE MEANS: a suspended or revoked company could keep operating, or roles get mixed up.
   */
  it('chooses the workspace from the wallet registration status and role', () => {
    expect(chooseSession('W', [party(6)])).toEqual({
      kind: 'signed-in',
      session: { role: 'slaughterhouse', wallet: 'W', partyId: '07'.repeat(32) },
    })
    expect(chooseSession('W', [party(10), party(1, PARTY_STATUS.ACTIVE, '08')])).toMatchObject({
      session: { role: 'producer', partyId: '08'.repeat(32) },
    })
    expect(chooseSession('W', [party(10)])).toMatchObject({ session: { role: 'viewer' } })
    expect(chooseSession('W', [])).toEqual({
      kind: 'signed-in',
      session: { role: 'viewer', wallet: 'W', partyId: null },
    })
    expect(chooseSession('W', [party(6, PARTY_STATUS.SUSPENDED)])).toEqual({
      kind: 'blocked',
      status: 'suspended',
    })
    expect(chooseSession('W', [party(6, PARTY_STATUS.REVOKED)])).toEqual({
      kind: 'blocked',
      status: 'revoked',
    })
    expect(chooseSession('W', [party(6, PARTY_STATUS.REVOKED), party(5)])).toMatchObject({
      session: { role: 'carrier' },
    })
  })

  /**
   * ARRANGE: a connected wallet whose signer returns a valid signature, and one whose signer
   *          returns a signature made by a different key.
   * ACTION: run the full login.
   * ASSERT: the honest wallet is signed in from its party; the mismatched signer is refused
   *         before any party lookup happens.
   * FAILURE MEANS: a wallet extension could claim an address it cannot sign for.
   */
  it('proves control of the wallet before reading its party', async () => {
    const owner = walletKey()
    const intruder = walletKey()
    let lookups = 0
    const deps = {
      connectedWallet: () => owner.wallet,
      fetchParties: async () => {
        lookups += 1
        return []
      },
      domain: 'lastro.dev.br',
      nonce: () => 'n',
      now: () => new Date('2026-10-11T00:00:00Z'),
    }

    await expect(
      loginWithConnectedWallet({
        ...deps,
        signMessage: async (message) => ed25519.sign(message, owner.secretKey),
      }),
    ).resolves.toEqual({
      kind: 'signed-in',
      session: { role: 'viewer', wallet: owner.wallet, partyId: null },
    })
    expect(lookups).toBe(1)

    await expect(
      loginWithConnectedWallet({
        ...deps,
        signMessage: async (message) => ed25519.sign(message, intruder.secretKey),
      }),
    ).rejects.toThrow('does not prove control')
    expect(lookups).toBe(1)

    await expect(
      loginWithConnectedWallet({
        ...deps,
        connectedWallet: () => null,
        signMessage: async () => new Uint8Array(),
      }),
    ).rejects.toThrow('Connect a Solana wallet')
  })
})
