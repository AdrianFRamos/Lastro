<!-- Operator console. State shown as canonical advances only after finalized Solana confirmation. -->
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
        <h1>Physical evidence → canonical identity and custody</h1>
        <p>
          Register an animal on Solana, bind its RFID with Station evidence, replace a lost tag,
          prove presence and transfer custody — each step authorized by the right wallet and shown
          only after finalized Solana state.
        </p>
      </div>
    </div>

    <div class="grid">
      <AnimalState :asset="asset" />
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
                Availability follows the canonical asset state; disabled actions explain what is
                missing.
              </p>
            </div>
          </header>

          <div class="action-grid">
            <article class="action-card" data-kind="bind">
              <span class="action-card__type">BIND RFID</span>
              <h3>Bind physical identity</h3>
              <p>{{ bindAvailability }}</p>
              <AppButton
                :disabled="busy || !canBind"
                :busy="busy && canBind"
                @click="runCapture('BIND_IDENTIFIER')"
                >Bind RFID</AppButton
              >
            </article>

            <article class="action-card" data-kind="replace">
              <span class="action-card__type">REPLACE RFID</span>
              <h3>Replace a lost or damaged tag</h3>
              <p>{{ replaceAvailability }}</p>
              <AppButton
                :disabled="busy || !canReplace"
                :busy="busy && canReplace"
                @click="runCapture('REPLACE_IDENTIFIER')"
                >Replace RFID</AppButton
              >
            </article>

            <article class="action-card" data-kind="observe">
              <span class="action-card__type">PRESENCE</span>
              <h3>Prove physical presence</h3>
              <p>{{ observeAvailability }}</p>
              <AppButton
                :disabled="busy || !canObserve"
                :busy="busy && canObserve"
                @click="runCapture('OBSERVE_PRESENCE')"
                >Record presence</AppButton
              >
            </article>

            <article class="action-card" data-kind="custody">
              <span class="action-card__type">CUSTODY</span>
              <h3>Two-phase custody transfer</h3>
              <p>{{ custodyAvailability }}</p>
              <div class="control action-card__control">
                <label for="operator-token">Operator token</label>
                <input
                  id="operator-token"
                  v-model.trim="operatorToken"
                  class="input mono"
                  type="password"
                  autocomplete="off"
                  :disabled="busy"
                />
                <label for="recipient-party">Recipient party ID</label>
                <input
                  id="recipient-party"
                  v-model.trim="recipientPartyId"
                  class="input mono"
                  autocomplete="off"
                  :disabled="busy"
                />
                <label for="recipient-facility">Recipient facility ID</label>
                <input
                  id="recipient-facility"
                  v-model.trim="recipientFacilityId"
                  class="input mono"
                  autocomplete="off"
                  :disabled="busy"
                />
                <label for="transfer-id">Transfer ID (to accept)</label>
                <input
                  id="transfer-id"
                  v-model.trim="pendingTransferId"
                  class="input mono"
                  autocomplete="off"
                  placeholder="Filled after a proposal, or paste one"
                  :disabled="busy"
                />
                <label for="recipient-wallet">Recipient wallet</label>
                <input
                  id="recipient-wallet"
                  v-model.trim="recipientWallet"
                  class="input mono"
                  autocomplete="off"
                  placeholder="Wallet address"
                  :disabled="busy"
                />
              </div>
              <AppButton
                :disabled="busy || !canProposeCustody"
                :busy="busy && canProposeCustody"
                @click="proposeCustody"
                >Propose (current custodian)</AppButton
              >
              <AppButton
                :disabled="busy || !canAcceptCustody"
                :busy="busy && canAcceptCustody"
                @click="acceptCustody"
                >Accept (recipient wallet)</AppButton
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
              <p class="eyebrow">ANIMAL</p>
              <h2>Register or look up</h2>
              <p class="app-card__description">
                Registration is signed by the deployment authority wallet; the custodian wallet
                below receives custody.
              </p>
            </div>
          </header>

          <div class="control">
            <label for="custodian-wallet">Initial custodian wallet</label>
            <input
              id="custodian-wallet"
              v-model.trim="custodianWallet"
              class="input mono"
              autocomplete="off"
              placeholder="Defaults to the connected wallet"
              :disabled="busy"
            />
          </div>
          <div class="control-row control-row--spaced">
            <AppButton
              :disabled="busy || walletAddress === null"
              :busy="busy"
              @click="registerAsset"
              >Register animal</AppButton
            >
          </div>

          <div class="control">
            <label for="asset-lookup">AssetID or RFID hash</label>
            <input
              id="asset-lookup"
              v-model.trim="lookupId"
              class="input mono"
              aria-label="AssetID or RFID hash"
              autocomplete="off"
              placeholder="64-character hex"
              :disabled="busy"
            />
          </div>
          <div class="control-row control-row--spaced">
            <AppButton variant="secondary" :disabled="busy || !lookupValid" @click="loadAsset"
              >Load by AssetID</AppButton
            >
            <AppButton variant="secondary" :disabled="busy || !lookupValid" @click="loadByRfid"
              >Find by RFID hash</AppButton
            >
          </div>
          <p class="control__help control__help--spaced">
            A replaced tag still resolves to its animal: RFID bindings are retired, never reused.
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

            <RouterLink v-if="asset" class="button" :to="'/verify/' + asset.assetId">
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
import type { AssetProjection, Capture, CaptureAction, Hex32 } from '../api/types'
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
import type { EvidencePackage } from '../protocol/evidence'
import { toHex } from '../protocol/hash'
import { V2EventType } from '../protocol/v2/domainEvent'
import {
  custodianAddress,
  rebroadcastSignedLastroTransaction,
  submitLastroTransaction,
} from '../solana/transaction'
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

const POLL_INTERVAL_MS = 1_000
const EVIDENCE_POLL_ATTEMPTS = 120
const SUBMISSION_POLL_ATTEMPTS = 60
const CONFIRM_POLL_ATTEMPTS = 60
const ANIMAL_ASSET_TYPE = 1
const DEFAULT_LIVE_WEIGHT_GRAMS = 450_000

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

const ACTION_LABELS: Record<CaptureAction, string> = {
  BIND_IDENTIFIER: 'RFID binding',
  REPLACE_IDENTIFIER: 'RFID replacement',
  OBSERVE_PRESENCE: 'Presence proof',
}

const asset = ref<AssetProjection | null>(null)
const walletAddress = ref<string | null>(currentWalletAddress())
const walletChoices = ref<WalletChoice[]>([])
const selectedWalletName = ref('')
const custodianWallet = ref('')
const lookupId = ref('')
const operatorToken = ref('')
const recipientPartyId = ref('')
const recipientFacilityId = ref('')
const recipientWallet = ref('')
const pendingTransferId = ref('')
const captureStatus = ref('none')
const busy = ref(false)
const message = ref('Register or load an animal to begin.')
const lastTransactionSignature = ref<string | null>(null)
const timeline = ref<Array<{ sequence: number; label: string }>>([])
const operationStage = ref<OperationStage>('IDLE')

const lookupValid = computed(() => /^[0-9a-f]{64}$/.test(lookupId.value))
const connectedCustodian = computed(() =>
  walletAddress.value === null ? null : walletAddressToCustodianHex(walletAddress.value),
)
const isCustodian = computed(
  () => asset.value !== null && asset.value.custodian === connectedCustodian.value,
)
const tagged = computed(() => asset.value?.currentRfidHash != null)
const canBind = computed(() => isCustodian.value && !tagged.value)
const canReplace = computed(() => isCustodian.value && tagged.value)
const canObserve = computed(() => isCustodian.value && tagged.value)
const canProposeCustody = computed(
  () =>
    isCustodian.value &&
    operatorToken.value.length > 0 &&
    /^[0-9a-f]{64}$/.test(recipientPartyId.value) &&
    /^[0-9a-f]{64}$/.test(recipientFacilityId.value) &&
    recipientWallet.value.length > 0,
)
const canAcceptCustody = computed(
  () =>
    asset.value !== null &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(
      pendingTransferId.value,
    ) &&
    walletAddress.value !== null,
)
const canRunStaleCustodianAttempt = computed(
  () => asset.value !== null && walletAddress.value !== null && !isCustodian.value,
)

const bindAvailability = computed(() => {
  if (!asset.value) return 'Register or load an animal first.'
  if (tagged.value) return 'This animal already has an active RFID; replace it instead.'
  if (!isCustodian.value) return 'Connect the current custodian wallet.'
  return 'Fresh Station evidence binds the first RFID to this AssetID.'
})
const replaceAvailability = computed(() => {
  if (!asset.value || !tagged.value) return 'Requires an animal with an active RFID.'
  if (!isCustodian.value) return 'Connect the current custodian wallet.'
  return 'The old tag is retired (it still resolves here); a new tag becomes active.'
})
const observeAvailability = computed(() => {
  if (!asset.value || !tagged.value) return 'Requires an animal with an active RFID.'
  if (!isCustodian.value) return 'Connect the current custodian wallet.'
  return 'The Station must read the active tag; the custodian anchors the proof.'
})
const custodyAvailability = computed(() => {
  if (!asset.value) return 'Register or load an animal first.'
  if (pendingTransferId.value)
    return `Transfer ${pendingTransferId.value}: the recipient wallet accepts it (share this ID).`
  if (!isCustodian.value) return 'Connect the current custodian wallet to propose.'
  return 'The custodian proposes; the recipient wallet accepts on-chain.'
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
    case 'WALLET_AUTHORIZATION':
      return 'WAITING'
    case 'EVIDENCE_SIGNED':
      return 'SIGNED'
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

async function registerAsset(): Promise<void> {
  await guarded(async () => {
    const connected = requireWallet()
    const custodian = walletAddressToCustodianHex(custodianWallet.value || connected)
    const assetId = toHex(crypto.getRandomValues(new Uint8Array(32)))
    const registration = {
      assetId,
      custodian,
      assetType: ANIMAL_ASSET_TYPE,
      availableWeightGrams: DEFAULT_LIVE_WEIGHT_GRAMS,
    }
    operationStage.value = 'WALLET_AUTHORIZATION'
    message.value = 'Sign the register_asset transaction with the deployment authority wallet.'
    const transactionData = await api.getRegisterAssetTransaction(registration)
    const signature = await submitLastroTransaction(transactionData, {
      kind: 'REGISTER_ASSET',
      ...registration,
    })
    lastTransactionSignature.value = signature
    operationStage.value = 'SUBMITTED'
    message.value = 'Registration broadcast. Waiting for the finalized AssetState.'
    await activateAsset(await pollFinalized(() => api.getAsset(assetId), isNotFound))
    operationStage.value = 'FINALIZED'
    message.value = `Animal ${assetId} registered on Solana. Bind its RFID next.`
  }, true)
}

async function loadAsset(): Promise<void> {
  await guarded(async () => {
    await activateAsset(await api.getAsset(lookupId.value))
    message.value = `Loaded AssetID ${lookupId.value} from canonical Solana state.`
  })
}

async function loadByRfid(): Promise<void> {
  await guarded(async () => {
    const found = await api.getAssetByRfid(lookupId.value)
    await activateAsset(found.asset)
    message.value =
      found.bindingStatus === 'ACTIVE'
        ? `RFID is the active tag of AssetID ${found.asset.assetId}.`
        : `RFID is a RETIRED tag; it still identifies AssetID ${found.asset.assetId}.`
  })
}

async function runCapture(action: CaptureAction): Promise<void> {
  await guarded(async () => {
    const selected = asset.value
    if (!selected) throw new Error('Select an animal before starting a capture')
    requireWallet()
    operationStage.value = 'WALLET_AUTHORIZATION'
    message.value = 'Authorize this exact capture intent before reserving physical Station work.'
    const challenge = await api.getCaptureAuthorizationChallenge(action, selected.assetId)
    const authorization = await signCaptureAuthorization(challenge, {
      action,
      assetId: selected.assetId,
      stateVersion: selected.stateVersion + 1,
    })
    const capture = await api.createCapture(action, selected.assetId, authorization)
    const pending: PendingOperation = {
      assetId: selected.assetId,
      captureId: capture.captureId,
      action,
      eventHash: capture.eventHash,
      txSignature: capture.txSignature,
      wireTransactionBase64: null,
      lastValidBlockHeight: null,
    }
    writePendingOperation(pending)
    captureStatus.value = formatCaptureStatus(capture)
    message.value =
      capture.status === 'EVIDENCE_ACCEPTED'
        ? 'Existing Station evidence recovered. Resuming the same immutable transition.'
        : `Capture ${capture.captureId} is waiting for the Station to read the RFID.`
    const accepted = await waitForEvidence(capture)
    await completeAcceptedCapture(accepted, pending, challenge.requiredSigner)
  }, true)
}

async function completeAcceptedCapture(
  accepted: Capture,
  pending: PendingOperation,
  requiredSigner: string,
): Promise<void> {
  const eventHash = accepted.eventHash
  if (!eventHash) throw new Error('Accepted capture did not expose its immutable eventHash')
  captureStatus.value = formatCaptureStatus(accepted)
  operationStage.value = 'EVIDENCE_SIGNED'

  let txSignature = accepted.txSignature ?? pending.txSignature
  let wireTransactionBase64 = pending.wireTransactionBase64
  if (!txSignature) {
    message.value = 'Station evidence accepted. Preparing the exact wallet transaction.'
    const transactionData = await api.getTransactionData(eventHash)
    operationStage.value = 'WALLET_AUTHORIZATION'
    txSignature = await submitLastroTransaction(
      transactionData,
      {
        kind: 'STATION_EVENT',
        action: accepted.action,
        assetId: accepted.assetId,
        eventHash,
        requiredSigner,
      },
      (signed) => {
        wireTransactionBase64 = signed.wireTransactionBase64
        writePendingOperation({
          ...pending,
          eventHash,
          txSignature: signed.signature,
          wireTransactionBase64: signed.wireTransactionBase64,
          lastValidBlockHeight: signed.lastValidBlockHeight?.toString() ?? null,
        })
      },
    )
  }

  lastTransactionSignature.value = txSignature
  if (accepted.eventStatus !== 'SUBMITTED' && accepted.eventStatus !== 'FINALIZED') {
    message.value = 'Transaction broadcast. Waiting for confirmed transaction verification.'
    await waitForSubmitted(
      eventHash,
      txSignature,
      wireTransactionBase64,
      readPendingOperation()?.lastValidBlockHeight,
    )
  }

  operationStage.value = 'SUBMITTED'
  message.value = 'Transaction verified as submitted. Waiting for finalized canonical Solana state.'
  const finalized = await pollFinalized(
    () => api.confirm(eventHash, txSignature!),
    isFinalityPending,
  )
  if (finalized.status !== 'FINALIZED') throw new Error('API did not report a finalized event')
  await activateAsset(await api.getAsset(accepted.assetId))
  clearPendingOperation()
  operationStage.value = 'FINALIZED'
  message.value = `${ACTION_LABELS[accepted.action]} finalized and canonical state verified.`
}

async function proposeCustody(): Promise<void> {
  await guarded(async () => {
    const selected = asset.value
    if (!selected) throw new Error('Select an animal first')
    requireWallet()
    const recipient = walletAddressToCustodianHex(recipientWallet.value)
    const transferId = crypto.randomUUID()
    await api.createCustodyTransfer(operatorToken.value, {
      transferId,
      assetId: selected.assetId,
      fromPartyId: null,
      toPartyId: recipientPartyId.value,
      fromFacilityId: null,
      toFacilityId: recipientFacilityId.value,
      reason: 'Operator demo custody transfer',
      createdByPartyId: null,
    })
    operationStage.value = 'WALLET_AUTHORIZATION'
    message.value = 'Sign the custody proposal with the current custodian wallet.'
    const transactionData = await api.getCustodyTransferTransaction(transferId, 'propose')
    lastTransactionSignature.value = await submitLastroTransaction(transactionData, {
      kind: 'CUSTODY_PROPOSE',
      assetId: selected.assetId,
      transferId,
      recipient,
    })
    pendingTransferId.value = transferId
    operationStage.value = 'SUBMITTED'
    message.value =
      'Proposal broadcast. After it finalizes, connect the recipient wallet and press Accept.'
  }, true)
}

async function acceptCustody(): Promise<void> {
  await guarded(async () => {
    const selected = asset.value
    const transferId = pendingTransferId.value
    if (!selected || !transferId) throw new Error('Propose a custody transfer first')
    const recipient = requireWallet()
    operationStage.value = 'WALLET_AUTHORIZATION'
    message.value = 'Sign the acceptance with the recipient wallet.'
    const transactionData = await api.getCustodyTransferTransaction(transferId, 'accept')
    const signature = await submitLastroTransaction(transactionData, {
      kind: 'CUSTODY_ACCEPT',
      assetId: selected.assetId,
      transferId,
      // The connected wallet accepts for itself; the browser rejects a transfer to anyone else.
      recipient: walletAddressToCustodianHex(recipient),
    })
    lastTransactionSignature.value = signature
    operationStage.value = 'SUBMITTED'
    message.value = 'Acceptance broadcast. Waiting for the finalized custodian change.'
    await pollFinalized(() => api.acceptCustodyTransfer(transferId, signature), isCustodyPending)
    pendingTransferId.value = ''
    await activateAsset(await api.getAsset(selected.assetId))
    operationStage.value = 'FINALIZED'
    message.value = 'Custody transfer finalized: the recipient is the canonical custodian.'
  }, true)
}

async function runInvalidOldCustodianAttempt(): Promise<void> {
  await guarded(async () => {
    if (!asset.value || walletAddress.value === null)
      throw new Error('A registered animal and connected wallet are required')
    if (isCustodian.value) {
      throw new Error(
        'Connected wallet is the current custodian. Switch to a previous custodian wallet to run the stale-authority check.',
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
        await rebroadcastSignedLastroTransaction({ signature: txSignature, wireTransactionBase64 })
        rebroadcasted = true
        continue
      }
      await delay(POLL_INTERVAL_MS)
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

/** Retry `load` while `pending(error)` says finality has not arrived yet. */
async function pollFinalized<T>(
  load: () => Promise<T>,
  pending: (error: unknown) => boolean,
): Promise<T> {
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await load()
    } catch (error) {
      if (!pending(error) || attempt >= CONFIRM_POLL_ATTEMPTS - 1) throw error
      await delay(POLL_INTERVAL_MS)
    }
  }
}

function isSubmissionPending(error: unknown): boolean {
  return (
    conflictMessage(error) ===
    'transaction is not the confirmed exact transaction for this v2 event'
  )
}

/** 503: the API could not prove finality yet (e.g. the transaction is not finalized). Transient. */
function isUnavailable(error: unknown): boolean {
  return error instanceof ApiClientError && error.status === 503
}

function isFinalityPending(error: unknown): boolean {
  const reason = conflictMessage(error)
  return (
    isUnavailable(error) ||
    reason === 'transaction is not the finalized exact transaction for this v2 event' ||
    reason === 'finalized transaction did not produce EventAnchor'
  )
}

function isCustodyPending(error: unknown): boolean {
  const reason = conflictMessage(error)
  return (
    isUnavailable(error) ||
    reason === 'finalized Solana state does not show the recipient as custodian' ||
    reason === 'transaction is not the finalized recipient acceptance of this transfer'
  )
}

function isNotFound(error: unknown): boolean {
  return error instanceof ApiClientError && error.status === 404
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
    await delay(POLL_INTERVAL_MS)
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

function requireWallet(): string {
  const connected = currentWalletAddress()
  if (!connected) throw new Error('Connect the required wallet first')
  walletAddress.value = connected
  return connected
}

function refreshWalletChoices(): void {
  walletChoices.value = availableWalletChoices()
}

async function activateAsset(projection: AssetProjection): Promise<void> {
  asset.value = projection
  lookupId.value = projection.assetId
  rememberAsset(projection.assetId)
  timeline.value = timelineFromEvidence(await api.getEvidencePackage(projection.assetId))
}

function timelineFromEvidence(pkg: EvidencePackage): Array<{ sequence: number; label: string }> {
  const events = pkg.events.map((event) => {
    const sequence = event.stateVersion
    switch (event.eventType) {
      case V2EventType.IdentifierBound:
        return { sequence, label: `#${sequence} RFID BOUND — tag ${event.observedRfidHex}` }
      case V2EventType.IdentifierReplaced:
        return { sequence, label: `#${sequence} RFID REPLACED — new tag ${event.observedRfidHex}` }
      case V2EventType.ObservationRecorded:
        return { sequence, label: `#${sequence} PRESENCE PROVEN — tag ${event.observedRfidHex}` }
      default:
        return { sequence, label: `#${sequence} EVENT type ${event.eventType}` }
    }
  })
  const transfers = pkg.custodyTransfers.map((transfer, index) => ({
    sequence: events.length + index + 1,
    label: `CUSTODY → ${custodianAddress(transfer.newCustodian)}`,
  }))
  return [...events, ...transfers]
}

function rememberAsset(assetId: Hex32): void {
  const url = new URL(window.location.href)
  url.searchParams.set('assetId', assetId)
  window.history.replaceState(null, '', `${url.pathname}${url.search}${url.hash}`)
}

async function restoreRememberedSession(): Promise<void> {
  const candidate = new URL(window.location.href).searchParams.get('assetId')
  if (!candidate || !/^[0-9a-f]{64}$/.test(candidate)) return
  await guarded(async () => {
    const restored = await api.getAsset(candidate)
    await activateAsset(restored)
    const pending = readPendingOperation()
    if (!pending || pending.assetId !== restored.assetId) {
      message.value = `Restored AssetID ${restored.assetId} from canonical Solana state.`
      return
    }

    const capture = await api.getCapture(pending.captureId)
    if (capture.assetId !== pending.assetId || capture.action !== pending.action) {
      clearPendingOperation()
      throw new Error('Stored pending operation does not match the durable capture')
    }
    captureStatus.value = formatCaptureStatus(capture)
    if (capture.status === 'EXPIRED' || capture.status === 'CANCELLED') {
      clearPendingOperation()
      operationStage.value = 'ERROR'
      message.value = `Stored capture ended with status ${capture.status}. Start a new physical capture.`
      return
    }
    const accepted = await waitForEvidence(capture)
    const txSignature = accepted.txSignature ?? pending.txSignature
    if (!txSignature) {
      writePendingOperation({ ...pending, eventHash: accepted.eventHash })
      operationStage.value = 'WALLET_AUTHORIZATION'
      message.value =
        'Recovered accepted Station evidence. Connect the required wallet and repeat the same action to authorize it; no new RFID read is needed.'
      return
    }
    // A signature exists, so the signer is only needed to validate a fresh signing request.
    await completeAcceptedCapture(
      accepted,
      { ...pending, txSignature },
      custodianAddress(restored.custodian),
    )
  }, true)
}

onMounted(() => {
  refreshWalletChoices()
  void restoreRememberedSession()
})

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds))
}
</script>
