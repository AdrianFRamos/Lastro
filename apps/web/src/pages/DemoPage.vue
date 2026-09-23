<!-- Hackathon operator console. Canonical-looking state advances only after finalized Solana confirmation through the API. -->
<template>
  <main class="page page--workbench">
    <header class="app-header">
      <RouterLink class="app-header__brand" to="/" aria-label="Lastro home">
        <img src="/logo.svg" alt="" width="36" height="36" />
        <span>
          <strong>LASTRO</strong>
          <span class="app-header__context">Operator demo</span>
        </span>
      </RouterLink>

      <div class="app-header__tools">
        <span class="network-pill">{{ webConfig.solanaChain }}</span>
        <WalletStatus :address="walletAddress" :connected="walletAddress !== null" />
        <select
          v-model="selectedWalletName"
          class="select app-header__select"
          aria-label="Wallet Standard wallet"
          :disabled="busy"
        >
          <option value="">First available wallet</option>
          <option v-for="choice in walletChoices" :key="choice.name" :value="choice.name">
            {{ choice.name }} — {{ choice.address }}
          </option>
        </select>
        <AppButton :disabled="busy" :busy="busy" @click="connectWallet">Connect wallet</AppButton>
      </div>
    </header>

    <div class="page-intro">
      <div>
        <p class="eyebrow">SOFTWARE DEMO / OPERATOR WORKSPACE</p>
        <h1>Physical evidence → canonical custody</h1>
        <p>
          Run the hackathon proof end to end: resolve one animal, request Station evidence,
          authorize with the current custodian wallet, then wait for finalized Solana state.
        </p>
      </div>
    </div>

    <div class="grid">
      <AnimalState :animal="animal" />
      <StationPanel :capture="captureStatus" />
    </div>

    <div class="workspace">
      <div class="workspace__main stack">
        <CustodyTimeline :events="timeline" />

        <AppCard>
          <header class="app-card__header">
            <div>
              <p class="eyebrow">ACTIONS</p>
              <h2>Canonical transitions</h2>
              <p class="app-card__description">
                Availability comes from the existing protocol prerequisites; disabled actions
                explain what is missing.
              </p>
            </div>
          </header>

          <div class="action-grid">
            <article class="action-card" data-kind="origin">
              <span class="action-card__type">ORIGIN</span>
              <h3>Bind physical identity</h3>
              <p>{{ originAvailability }}</p>
              <AppButton
                :disabled="busy || !canOrigin"
                :busy="busy && canOrigin"
                @click="runAction('ORIGIN')"
                >Origin</AppButton
              >
            </article>

            <article class="action-card" data-kind="transfer">
              <span class="action-card__type">TRANSFER</span>
              <h3>Change custody</h3>
              <p>{{ transferAvailability }}</p>
              <div class="control action-card__control">
                <label for="next-custodian">Next custodian wallet</label>
                <input
                  id="next-custodian"
                  v-model.trim="nextCustodianAddress"
                  class="input mono"
                  aria-label="Next custodian wallet"
                  autocomplete="off"
                  placeholder="Wallet address"
                  :disabled="busy"
                />
              </div>
              <AppButton
                :disabled="busy || !canTransfer"
                :busy="busy && canTransfer"
                @click="runAction('TRANSFER')"
                >Transfer</AppButton
              >
            </article>

            <article class="action-card" data-kind="reidentify">
              <span class="action-card__type">REIDENTIFY</span>
              <h3>Replace RFID binding</h3>
              <p>{{ reidentifyAvailability }}</p>
              <AppButton
                :disabled="busy || !canReidentify"
                :busy="busy && canReidentify"
                @click="runAction('REIDENTIFY')"
                >Reidentify</AppButton
              >
              <AppButton
                v-if="canRescanReidentify"
                :disabled="busy"
                @click="runAction('REIDENTIFY', true)"
                >Retry RFID scan</AppButton
              >
            </article>
          </div>

          <div v-if="canRunStaleCustodianAttempt" class="identity-resolution">
            <p>
              Connected wallet is not the current custodian. The explicit stale-authority check
              creates no capture, evidence or transaction.
            </p>
            <AppButton variant="danger" :disabled="busy" @click="runInvalidOldCustodianAttempt"
              >Run stale-custodian attempt</AppButton
            >
          </div>
        </AppCard>
      </div>

      <aside class="workspace__aside stack" aria-label="Operator controls and operation status">
        <AppCard>
          <header class="app-card__header">
            <div>
              <p class="eyebrow">ANIMAL LOOKUP</p>
              <h2>Create or recover</h2>
              <p class="app-card__description">
                The visual recovery identifier resolves digital identity; it never grants custody
                authority.
              </p>
            </div>
          </header>

          <div class="control">
            <label for="visual-recovery-id">Visual recovery ID</label>
            <input
              id="visual-recovery-id"
              v-model.trim="visualRecoveryId"
              class="input"
              aria-label="Visual recovery ID"
              autocomplete="off"
              placeholder="e.g. VIS-0042"
              :disabled="busy"
            />
            <p class="control__help">
              Use the independent recovery identifier when the current RFID is unavailable.
            </p>
          </div>

          <div class="control-row control-row--spaced">
            <AppButton :disabled="busy || !visualRecoveryId" :busy="busy" @click="createAnimal"
              >Create Animal</AppButton
            >
            <AppButton
              variant="secondary"
              :disabled="busy || !visualRecoveryId"
              :busy="busy"
              @click="recoverAnimal"
              >Find by visual recovery ID</AppButton
            >
          </div>

          <div class="identity-resolution">
            <p>
              <strong>Physical identity continuity:</strong>
              {{ animal ? 'RESOLVED' : 'UNRESOLVED' }}
            </p>
            <span class="status-badge" :data-tone="animal ? 'proof' : undefined">{{
              animal ? 'RESOLVED' : 'UNRESOLVED'
            }}</span>
          </div>

          <p v-if="!animal" class="control__help control__help--spaced">
            Provide the independent visual recovery identifier, or resolve the current RFID through
            an authoritative RFID lookup, before starting a transition. Lastro does not guess
            identity when both physical identifiers are unavailable.
          </p>
        </AppCard>

        <AppCard>
          <div class="operation-status">
            <div class="operation-status__headline">
              <div>
                <p class="eyebrow">CURRENT OPERATION / STATUS</p>
                <h2>{{ operationTitle }}</h2>
                <p>Canonical-looking state changes only after finalized confirmation.</p>
              </div>
              <span class="status-badge" :data-tone="operationTone">{{ operationLabel }}</span>
            </div>

            <ol class="operation-steps" aria-label="Operation lifecycle">
              <li
                v-for="(step, index) in operationSteps"
                :key="step.stage"
                class="operation-step"
                :data-state="operationStepState(index)"
              >
                <strong>0{{ index + 1 }}</strong>
                {{ step.label }}
              </li>
            </ol>

            <p
              class="status-message"
              :data-tone="
                operationStage === 'ERROR'
                  ? 'danger'
                  : operationStage === 'FINALIZED'
                    ? 'success'
                    : undefined
              "
              :aria-live="operationStage === 'ERROR' ? 'assertive' : 'polite'"
            >
              {{ message }}
            </p>

            <dl v-if="lastTransactionSignature" class="data-list">
              <dt>Last transaction</dt>
              <dd class="mono break-all">{{ lastTransactionSignature }}</dd>
            </dl>

            <RouterLink
              v-if="animal?.eventSequence"
              class="button"
              :to="'/verify/' + animal.animalId"
            >
              Verify current EvidencePackage
            </RouterLink>
          </div>
        </AppCard>
      </aside>
    </div>
  </main>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { api, ApiClientError } from '../api/client'
import type { AnimalProjection, Capture, CaptureAction, Hex32 } from '../api/types'
import AnimalState from '../components/AnimalState.vue'
import AppButton from '../components/AppButton.vue'
import AppCard from '../components/AppCard.vue'
import CustodyTimeline from '../components/CustodyTimeline.vue'
import StationPanel from '../components/StationPanel.vue'
import WalletStatus from '../components/WalletStatus.vue'
import { webConfig } from '../config'
import {
  clearPendingOperation,
  readPendingOperation,
  writePendingOperation,
  type PendingOperation,
} from '../demo/pendingOperation'
import { decodeStationEvent } from '../protocol/stationEvent'
import type { EvidencePackage } from '../protocol/evidence'
import { rebroadcastSignedLastroTransaction, submitLastroTransaction } from '../solana/transaction'
import { solanaClient } from '../solana/client'
import {
  availableWalletChoices,
  connectFirstAvailableWallet,
  connectWalletByName,
  currentWalletAddress,
  signCaptureAuthorization,
  walletAddressToCustodianHex,
  type WalletChoice,
} from '../solana/wallet'

const EVIDENCE_POLL_INTERVAL_MS = 1_000
const EVIDENCE_POLL_ATTEMPTS = 120
const SUBMISSION_POLL_ATTEMPTS = 60
const CONFIRM_POLL_ATTEMPTS = 60

type OperationStage =
  | 'IDLE'
  | 'WAITING_STATION'
  | 'EVIDENCE_SIGNED'
  | 'WALLET_AUTHORIZATION'
  | 'SUBMITTED'
  | 'FINALIZED'
  | 'ERROR'
type OperationTone = 'primary' | 'proof' | 'chain' | 'success' | 'danger' | undefined

const operationSteps = [
  { stage: 'WAITING_STATION', label: 'Waiting for Station' },
  { stage: 'EVIDENCE_SIGNED', label: 'Evidence signed' },
  { stage: 'WALLET_AUTHORIZATION', label: 'Wallet authorization' },
  { stage: 'SUBMITTED', label: 'Submitted' },
  { stage: 'FINALIZED', label: 'Finalized' },
] as const

const animal = ref<AnimalProjection | null>(null)
const walletAddress = ref<string | null>(currentWalletAddress())
const walletChoices = ref<WalletChoice[]>([])
const selectedWalletName = ref('')
const visualRecoveryId = ref('')
const nextCustodianAddress = ref('')
const captureStatus = ref('none')
const busy = ref(false)
const message = ref('Create or recover an animal to begin.')
const lastTransactionSignature = ref<string | null>(null)
const timeline = ref<Array<{ sequence: number; label: string }>>([])
const operationStage = ref<OperationStage>('IDLE')

const connectedCustodian = computed(() =>
  walletAddress.value === null ? null : walletAddressToCustodianHex(walletAddress.value),
)
const canOrigin = computed(
  () =>
    animal.value !== null &&
    animal.value.eventSequence === 0 &&
    animal.value.currentCustodian === null &&
    animal.value.currentRfidHash === null &&
    connectedCustodian.value !== null,
)
const canTransfer = computed(
  () =>
    animal.value !== null &&
    animal.value.eventSequence > 0 &&
    connectedCustodian.value !== null &&
    animal.value.currentCustodian === connectedCustodian.value &&
    nextCustodianAddress.value.length > 0,
)
const canReidentify = computed(
  () =>
    animal.value !== null &&
    animal.value.eventSequence > 0 &&
    connectedCustodian.value !== null &&
    animal.value.currentCustodian === connectedCustodian.value,
)
const canRescanReidentify = computed(() => {
  const pending = readPendingOperation()
  return (
    canReidentify.value &&
    pending?.action === 'REIDENTIFY' &&
    pending.animalId === animal.value?.animalId &&
    pending.eventHash !== null &&
    pending.txSignature === null &&
    (operationStage.value === 'ERROR' || operationStage.value === 'WALLET_AUTHORIZATION')
  )
})
const canRunStaleCustodianAttempt = computed(
  () =>
    animal.value !== null &&
    animal.value.eventSequence > 0 &&
    animal.value.currentCustodian !== null &&
    connectedCustodian.value !== null &&
    animal.value.currentCustodian !== connectedCustodian.value,
)

const originAvailability = computed(() => {
  if (!animal.value) return 'Select or recover an animal first.'
  if (
    animal.value.eventSequence !== 0 ||
    animal.value.currentCustodian !== null ||
    animal.value.currentRfidHash !== null
  ) {
    return 'Available only before the first canonical event.'
  }
  if (!connectedCustodian.value) return 'Connect the intended custodian wallet.'
  return 'Fresh Station evidence will bind the first RFID and custodian.'
})

const transferAvailability = computed(() => {
  if (!animal.value || animal.value.eventSequence === 0) return 'Requires an originated animal.'
  if (!connectedCustodian.value) return 'Connect the current custodian wallet.'
  if (animal.value.currentCustodian !== connectedCustodian.value)
    return 'Connected wallet is not the current custodian.'
  if (!nextCustodianAddress.value) return 'Enter the next custodian wallet.'
  return 'Fresh Station evidence and current wallet authority are required.'
})

const reidentifyAvailability = computed(() => {
  if (!animal.value || animal.value.eventSequence === 0) return 'Requires an originated animal.'
  if (!connectedCustodian.value) return 'Connect the current custodian wallet.'
  if (animal.value.currentCustodian !== connectedCustodian.value)
    return 'Connected wallet is not the current custodian.'
  return 'Keeps AnimalID and custody while RFID changes and revision advances.'
})

const operationTitle = computed(() => {
  switch (operationStage.value) {
    case 'WAITING_STATION':
      return 'Waiting for physical evidence'
    case 'EVIDENCE_SIGNED':
      return 'Station evidence signed'
    case 'WALLET_AUTHORIZATION':
      return 'Wallet authorization required'
    case 'SUBMITTED':
      return 'Waiting for Solana finality'
    case 'FINALIZED':
      return 'Canonical state finalized'
    case 'ERROR':
      return 'Operation needs attention'
    default:
      return 'Idle'
  }
})

const operationLabel = computed(() => {
  switch (operationStage.value) {
    case 'WAITING_STATION':
      return 'WAITING'
    case 'EVIDENCE_SIGNED':
      return 'SIGNED'
    case 'WALLET_AUTHORIZATION':
      return 'WAITING'
    case 'SUBMITTED':
      return 'SUBMITTED'
    case 'FINALIZED':
      return 'FINALIZED'
    case 'ERROR':
      return 'ERROR'
    default:
      return 'READY'
  }
})

const operationTone = computed<OperationTone>(() => {
  switch (operationStage.value) {
    case 'EVIDENCE_SIGNED':
      return 'proof'
    case 'WALLET_AUTHORIZATION':
      return 'primary'
    case 'SUBMITTED':
      return 'chain'
    case 'FINALIZED':
      return 'success'
    case 'ERROR':
      return 'danger'
    default:
      return undefined
  }
})

function operationStepState(index: number): 'pending' | 'active' | 'complete' {
  if (operationStage.value === 'IDLE' || operationStage.value === 'ERROR') return 'pending'
  const currentIndex = operationSteps.findIndex((step) => step.stage === operationStage.value)
  if (index < currentIndex) return 'complete'
  if (index === currentIndex) return operationStage.value === 'FINALIZED' ? 'complete' : 'active'
  return 'pending'
}

async function connectWallet(): Promise<void> {
  await guarded(async () => {
    refreshWalletChoices()
    if (selectedWalletName.value) await connectWalletByName(selectedWalletName.value)
    else await connectFirstAvailableWallet()
    walletAddress.value = currentWalletAddress()
    refreshWalletChoices()
    const connectedChoice = walletChoices.value.find(
      (choice) => choice.address === walletAddress.value,
    )
    if (connectedChoice) selectedWalletName.value = connectedChoice.name
    message.value = `Wallet connected: ${walletAddress.value}`
  })
}

async function createAnimal(): Promise<void> {
  await guarded(async () => {
    const created = await api.createAnimal(visualRecoveryId.value)
    await activateAnimal(created)
    message.value = 'Animal created. ORIGIN still requires physical RFID evidence.'
  })
}

async function recoverAnimal(): Promise<void> {
  await guarded(async () => {
    const recovered = await api.getAnimalByRecovery(visualRecoveryId.value)
    await activateAnimal(recovered)
    message.value = `Recovered AnimalID ${recovered.animalId} from the independent visual recovery identifier.`
  })
}

async function runAction(action: CaptureAction, rescan = false): Promise<void> {
  await guarded(async () => {
    const selected = animal.value
    if (!selected) throw new Error('Select an animal before starting a capture')
    let supersedeCaptureId: string | null = null
    if (rescan) {
      const previous = readPendingOperation()
      if (
        action !== 'REIDENTIFY' ||
        previous?.action !== 'REIDENTIFY' ||
        previous.animalId !== selected.animalId ||
        previous.eventHash === null ||
        previous.txSignature !== null
      ) {
        throw new Error('Only an unsigned accepted REIDENTIFY capture can be scanned again')
      }
      const persisted = await api.getCapture(previous.captureId)
      if (persisted.eventStatus !== 'EVIDENCE_ACCEPTED' || persisted.txSignature !== null) {
        throw new Error('The previous capture is already submitted or cannot be replaced')
      }
      supersedeCaptureId = previous.captureId
    }
    const connectedAddress = currentWalletAddress()
    if (!connectedAddress)
      throw new Error('Connect the custodian wallet before starting a protected action')
    walletAddress.value = connectedAddress

    const connectedCustodian = walletAddressToCustodianHex(connectedAddress)
    let nextCustodian: Hex32 | null = null
    if (action === 'ORIGIN') {
      nextCustodian = connectedCustodian
    } else if (action === 'TRANSFER') {
      nextCustodian = walletAddressToCustodianHex(nextCustodianAddress.value)
    }

    if (action !== 'ORIGIN' && selected.currentCustodian !== connectedCustodian) {
      throw new Error('Connected wallet is not the current custodian; capture was not created')
    }

    const expectedToCustodian = action === 'REIDENTIFY' ? selected.currentCustodian : nextCustodian
    if (!expectedToCustodian)
      throw new Error('Transition does not have an expected recipient custodian')

    operationStage.value = 'WALLET_AUTHORIZATION'
    message.value = 'Authorize this exact capture intent before reserving physical Station work.'
    const challenge = await api.getCaptureAuthorizationChallenge(
      action,
      selected.animalId,
      nextCustodian,
      supersedeCaptureId,
    )
    const authorization = await signCaptureAuthorization(challenge, {
      action,
      animalId: selected.animalId,
      nextCustodian,
      supersedeCaptureId,
    })
    const capture = await api.createCapture(
      action,
      selected.animalId,
      nextCustodian,
      authorization,
      supersedeCaptureId,
    )
    const pending: PendingOperation = {
      animalId: selected.animalId,
      captureId: capture.captureId,
      action,
      nextCustodian,
      expectedToCustodian,
      eventHash: capture.eventHash,
      txSignature: capture.txSignature,
      wireTransactionBase64: null,
      lastValidBlockHeight: null,
    }
    writePendingOperation(pending)
    captureStatus.value = formatCaptureStatus(capture)
    operationStage.value =
      capture.status === 'EVIDENCE_ACCEPTED' ? 'EVIDENCE_SIGNED' : 'WAITING_STATION'
    message.value =
      capture.status === 'EVIDENCE_ACCEPTED'
        ? 'Existing physical Station evidence recovered. Resuming the same immutable transition.'
        : `Capture ${capture.captureId} is waiting for physical Station RFID evidence.`

    const accepted = await waitForEvidence(capture)
    await completeAcceptedCapture(accepted, pending)
  }, true)
}

async function completeAcceptedCapture(
  accepted: Capture,
  pending: PendingOperation,
): Promise<void> {
  if (!accepted.eventHash)
    throw new Error('Accepted capture did not expose its immutable eventHash')
  captureStatus.value = formatCaptureStatus(accepted)
  operationStage.value = 'EVIDENCE_SIGNED'

  let txSignature = accepted.txSignature ?? pending.txSignature
  let wireTransactionBase64 = pending.wireTransactionBase64
  if (!txSignature) {
    message.value = 'Station evidence accepted. Preparing the exact wallet transaction.'
    const transactionData = await api.getTransactionData(accepted.eventHash)
    operationStage.value = 'WALLET_AUTHORIZATION'
    txSignature = await submitLastroTransaction(
      transactionData,
      {
        action: pending.action,
        animalId: pending.animalId,
        deploymentId: webConfig.lastroDeploymentId,
        toCustodian: pending.expectedToCustodian,
        eventHash: accepted.eventHash,
      },
      (signed) => {
        wireTransactionBase64 = signed.wireTransactionBase64
        writePendingOperation({
          ...pending,
          eventHash: accepted.eventHash,
          txSignature: signed.signature,
          wireTransactionBase64: signed.wireTransactionBase64,
          lastValidBlockHeight: signed.lastValidBlockHeight?.toString() ?? null,
        })
      },
    )
    writePendingOperation({
      ...(readPendingOperation() ?? pending),
      eventHash: accepted.eventHash,
      txSignature,
      wireTransactionBase64,
    })
  }

  lastTransactionSignature.value = txSignature
  if (accepted.eventStatus !== 'SUBMITTED' && accepted.eventStatus !== 'FINALIZED') {
    message.value =
      'Transaction broadcast. Waiting for exact transaction verification at confirmed commitment.'
    await waitForSubmitted(
      accepted.eventHash,
      txSignature,
      wireTransactionBase64,
      readPendingOperation()?.lastValidBlockHeight,
    )
  }

  operationStage.value = 'SUBMITTED'
  message.value = 'Transaction verified as submitted. Waiting for finalized canonical Solana state.'
  const confirmed = await confirmFinalized(accepted.eventHash, txSignature)
  await activateAnimal(confirmed)
  clearPendingOperation()
  nextCustodianAddress.value = ''
  operationStage.value = 'FINALIZED'
  message.value = `${accepted.action} finalized and canonical state verified.`
}

async function runInvalidOldCustodianAttempt(): Promise<void> {
  await guarded(async () => {
    const selected = animal.value
    const connectedAddress = currentWalletAddress()
    if (!selected?.currentCustodian || !connectedAddress)
      throw new Error('An originated animal and connected wallet are required')
    walletAddress.value = connectedAddress
    const connectedCustodian = walletAddressToCustodianHex(connectedAddress)
    if (connectedCustodian === selected.currentCustodian) {
      throw new Error(
        'Connected wallet is the current custodian. Switch to the previous custodian wallet to run the stale-authority check.',
      )
    }
    message.value =
      'REJECTED: connected wallet is not the current custodian. No capture, evidence, or transaction was created.'
  })
}

async function waitForSubmitted(
  eventHash: Hex32,
  txSignature: string,
  wireTransactionBase64?: PendingOperation['wireTransactionBase64'],
  lastValidBlockHeight?: string | null,
): Promise<void> {
  let rebroadcasted = false
  for (let attempt = 0; attempt < SUBMISSION_POLL_ATTEMPTS; attempt += 1) {
    try {
      const submission = await api.submit(eventHash, txSignature)
      if (submission.txSignature !== txSignature)
        throw new Error('API returned a different submitted transaction signature')
      if (submission.status !== 'SUBMITTED' && submission.status !== 'FINALIZED')
        throw new Error('API returned an invalid transaction submission status')
      return
    } catch (error) {
      if (!isSubmissionPending(error)) throw error
      if (
        lastValidBlockHeight &&
        (await signedTransactionExpired(txSignature, lastValidBlockHeight))
      ) {
        const pending = readPendingOperation()
        if (pending?.eventHash === eventHash && pending.txSignature === txSignature) {
          writePendingOperation({
            ...pending,
            txSignature: null,
            wireTransactionBase64: null,
            lastValidBlockHeight: null,
          })
        }
        throw new Error(
          'The signed Solana transaction expired without confirmation. Repeat the same action to sign the existing Station evidence with a fresh blockhash.',
        )
      }
      if (attempt === SUBMISSION_POLL_ATTEMPTS - 1) throw error
      if (!rebroadcasted && wireTransactionBase64) {
        message.value =
          'Confirmed transaction not found yet. Rebroadcasting the exact retained signed transaction.'
        await rebroadcastSignedLastroTransaction({
          signature: txSignature,
          wireTransactionBase64,
        })
        rebroadcasted = true
        continue
      }
      await delay(EVIDENCE_POLL_INTERVAL_MS)
    }
  }
  throw new Error('Confirmed Solana transaction did not arrive')
}

async function signedTransactionExpired(
  txSignature: string,
  lastValidBlockHeight: string,
): Promise<boolean> {
  const currentHeight = await solanaClient.rpc.getBlockHeight({ commitment: 'finalized' }).send()
  if (currentHeight <= BigInt(lastValidBlockHeight)) return false
  const statuses = await solanaClient.rpc
    .getSignatureStatuses([txSignature as import('@solana/kit').Signature], {
      searchTransactionHistory: true,
    })
    .send()
  // A live successful transaction can still finalize. A finalized failed transaction
  // cannot apply the event. The API revalidates the canonical predecessor on retry.
  const status = statuses.value[0]
  return status == null || (status.err !== null && status.confirmationStatus === 'finalized')
}

async function confirmFinalized(eventHash: Hex32, txSignature: string): Promise<AnimalProjection> {
  for (let attempt = 0; attempt < CONFIRM_POLL_ATTEMPTS; attempt += 1) {
    try {
      return await api.confirm(eventHash, txSignature)
    } catch (error) {
      if (!isFinalityPending(error) || attempt === CONFIRM_POLL_ATTEMPTS - 1) throw error
      await delay(EVIDENCE_POLL_INTERVAL_MS)
    }
  }
  throw new Error('Finalized Solana confirmation did not arrive')
}

function isSubmissionPending(error: unknown): boolean {
  return (
    conflictMessage(error) ===
    'transaction is not a confirmed exact Lastro transaction for this event'
  )
}

function isFinalityPending(error: unknown): boolean {
  return (
    conflictMessage(error) ===
    'transaction is not a finalized exact Lastro transaction for this event'
  )
}

function conflictMessage(error: unknown): string | null {
  if (!(error instanceof ApiClientError) || error.status !== 409 || !error.responseBody) return null
  try {
    const body = JSON.parse(error.responseBody) as { message?: unknown }
    return typeof body.message === 'string' ? body.message : null
  } catch {
    return null
  }
}

async function waitForEvidence(initial: Capture): Promise<Capture> {
  let current = initial
  for (let attempt = 0; attempt < EVIDENCE_POLL_ATTEMPTS; attempt += 1) {
    if (current.status === 'EVIDENCE_ACCEPTED') {
      operationStage.value = 'EVIDENCE_SIGNED'
      return current
    }
    if (current.status === 'EXPIRED' || current.status === 'CANCELLED') {
      throw new Error(`Capture ended with status ${current.status}`)
    }
    operationStage.value = 'WAITING_STATION'
    await delay(EVIDENCE_POLL_INTERVAL_MS)
    current = await api.getCapture(initial.captureId)
    captureStatus.value = formatCaptureStatus(current)
  }
  throw new Error('Timed out waiting for physical Station evidence')
}

function formatCaptureStatus(capture: Capture): string {
  return `${capture.status}${capture.eventStatus ? `/${capture.eventStatus}` : ''}: ${capture.captureId}`
}

function apiErrorMessage(error: unknown): string {
  if (error instanceof ApiClientError && error.responseBody) {
    try {
      const body = JSON.parse(error.responseBody) as { message?: unknown }
      if (typeof body.message === 'string') return body.message
    } catch {
      // fallback
    }
  }
  return error instanceof Error ? error.message : 'Operation failed'
}

async function guarded(work: () => Promise<void>, affectsOperation = false): Promise<void> {
  if (busy.value) return
  busy.value = true
  try {
    await work()
  } catch (error) {
    if (affectsOperation) operationStage.value = 'ERROR'
    message.value = apiErrorMessage(error)
  } finally {
    busy.value = false
  }
}

function refreshWalletChoices(): void {
  walletChoices.value = availableWalletChoices()
}

function activateAnimalProjection(projection: AnimalProjection): void {
  animal.value = projection
  visualRecoveryId.value = projection.visualRecoveryId
  rememberAnimal(projection.animalId)
}

async function activateAnimal(projection: AnimalProjection): Promise<void> {
  activateAnimalProjection(projection)
  timeline.value =
    projection.eventSequence === 0
      ? []
      : timelineFromEvidence(await api.getEvidencePackage(projection.animalId))
}

function timelineFromEvidence(pkg: EvidencePackage): Array<{ sequence: number; label: string }> {
  return pkg.events.map((evidence) => {
    const raw = Uint8Array.from(atob(evidence.eventBytesBase64), (character) =>
      character.charCodeAt(0),
    )
    const event = decodeStationEvent(raw)
    const sequence = Number(event.eventSequence)
    if (!Number.isSafeInteger(sequence))
      throw new Error('EvidencePackage event sequence exceeds browser safe integer range')
    if (event.action === 1)
      return { sequence, label: `#${sequence} ORIGIN — custodian ${hex(event.toCustodian)}` }
    if (event.action === 2)
      return { sequence, label: `#${sequence} TRANSFER — custodian ${hex(event.toCustodian)}` }
    return {
      sequence,
      label: `#${sequence} REIDENTIFY — RFID ${hex(event.newRfidHash)}, revision ${event.identityRevision}`,
    }
  })
}

function rememberAnimal(animalId: Hex32): void {
  const url = new URL(window.location.href)
  url.searchParams.set('animalId', animalId)
  window.history.replaceState(null, '', `${url.pathname}${url.search}${url.hash}`)
}

async function restoreRememberedSession(): Promise<void> {
  const candidate = new URL(window.location.href).searchParams.get('animalId')
  if (!candidate || !/^[0-9a-f]{64}$/.test(candidate)) return
  await guarded(async () => {
    const restored = await api.getAnimal(candidate)
    const pending = readPendingOperation()
    if (!pending || pending.animalId !== restored.animalId) {
      await activateAnimal(restored)
      message.value = `Restored AnimalID ${restored.animalId} from durable API projection and evidence history.`
      return
    }

    activateAnimalProjection(restored)
    timeline.value = []
    const capture = await api.getCapture(pending.captureId)
    if (capture.animalId !== pending.animalId || capture.action !== pending.action) {
      clearPendingOperation()
      throw new Error('Stored pending operation does not match the durable capture')
    }
    captureStatus.value = formatCaptureStatus(capture)
    operationStage.value =
      capture.status === 'EVIDENCE_ACCEPTED' ? 'EVIDENCE_SIGNED' : 'WAITING_STATION'
    if (capture.status === 'EXPIRED' || capture.status === 'CANCELLED') {
      clearPendingOperation()
      operationStage.value = 'ERROR'
      message.value = `Stored capture ended with status ${capture.status}. Start a new physical capture.`
      return
    }

    const accepted = await waitForEvidence(capture)
    if (!accepted.eventHash)
      throw new Error('Accepted capture did not expose its immutable eventHash')
    const txSignature = accepted.txSignature ?? pending.txSignature
    writePendingOperation({ ...pending, eventHash: accepted.eventHash, txSignature })
    if (!txSignature) {
      operationStage.value = 'WALLET_AUTHORIZATION'
      message.value =
        'Recovered accepted Station evidence. Connect the required wallet and repeat the same action to authorize it; no new RFID observation will be created.'
      return
    }

    lastTransactionSignature.value = txSignature
    if (accepted.eventStatus !== 'SUBMITTED' && accepted.eventStatus !== 'FINALIZED') {
      operationStage.value = 'WALLET_AUTHORIZATION'
      message.value =
        'Recovered a broadcast transaction. Waiting for confirmed transaction verification.'
      await waitForSubmitted(
        accepted.eventHash,
        txSignature,
        pending.wireTransactionBase64,
        pending.lastValidBlockHeight,
      )
    }
    operationStage.value = 'SUBMITTED'
    message.value = 'Recovered submitted transaction. Waiting for finalized canonical Solana state.'
    const confirmed = await confirmFinalized(accepted.eventHash, txSignature)
    await activateAnimal(confirmed)
    clearPendingOperation()
    operationStage.value = 'FINALIZED'
    message.value = `${accepted.action} finalized and canonical state verified after reload.`
  }, true)
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

onMounted(() => {
  refreshWalletChoices()
  void restoreRememberedSession()
})

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds))
}
</script>
