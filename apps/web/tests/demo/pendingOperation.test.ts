import { beforeEach, describe, expect, it } from 'vitest'
import {
  clearPendingOperation,
  readPendingOperation,
  writePendingOperation,
  type PendingOperation,
} from '../../src/demo/pendingOperation'

const TX_SIGNATURE = '2AXDGYSE4f2sz7tvMMzyHvUfcoJmxudvdhBcmiUSo6ijwfYmfZYsKRxboQMPh3R4kUhXRVdtSXFXMheka4Rc4P2'

const operation: PendingOperation = {
  animalId: '11'.repeat(32),
  captureId: '00112233-4455-6677-8899-aabbccddeeff',
  action: 'TRANSFER',
  nextCustodian: '22'.repeat(32),
  eventHash: '33'.repeat(32),
  txSignature: TX_SIGNATURE,
}

beforeEach(() => {
  window.localStorage.clear()
})

describe('demo/pendingOperation', () => {
  /**
   * PURPOSE: Preserve enough non-authoritative browser workflow state to resume an already broadcast transaction after reload.
   * ARRANGE: Store one valid pending operation containing capture, event, and transaction identifiers.
   * ACTION: Read it back from localStorage.
   * ASSERT: Every field round-trips exactly and clear removes it.
   * FAILURE MEANS: a reload can force another physical capture or wallet signature even though the same immutable transition is still in progress.
   */
  it('round-trips the pending operation exactly and clears it explicitly', () => {
    writePendingOperation(operation)
    expect(readPendingOperation()).toEqual(operation)
    clearPendingOperation()
    expect(readPendingOperation()).toBeNull()
  })

  /**
   * PURPOSE: Treat browser storage as untrusted workflow metadata rather than protocol truth.
   * ARRANGE: Put malformed/corrupted data under the pending-operation storage key.
   * ACTION: Read the pending operation.
   * ASSERT: Parsing fails closed, removes the corrupted value, and returns null.
   * FAILURE MEANS: attacker- or corruption-controlled localStorage can steer capture/event recovery with invalid identifiers.
   */
  it('fails closed and removes malformed local storage', () => {
    window.localStorage.setItem('lastro.pending-operation', JSON.stringify({ ...operation, eventHash: 'not-a-hash' }))
    expect(readPendingOperation()).toBeNull()
    expect(window.localStorage.getItem('lastro.pending-operation')).toBeNull()
  })
  /**
   * PURPOSE: Bound attacker-controlled localStorage identifiers used only for recovery hints.
   * ARRANGE: Corrupt captureId and txSignature independently with oversized/malformed text.
   * ACTION: Read the pending operation.
   * ASSERT: Each variant is discarded and storage is cleared.
   * FAILURE MEANS: localStorage can retain oversized or ambiguous identifiers across reload recovery.
   */
  it('rejects malformed or oversized durable identifiers from local storage', () => {
    for (const corrupted of [
      { ...operation, captureId: 'x'.repeat(4096) },
      { ...operation, txSignature: 'not-base58' },
      { ...operation, txSignature: '1'.repeat(89) },
    ]) {
      window.localStorage.setItem('lastro.pending-operation', JSON.stringify(corrupted))
      expect(readPendingOperation()).toBeNull()
      expect(window.localStorage.getItem('lastro.pending-operation')).toBeNull()
    }
  })

})
