/**
 * Step-by-step walkthrough of one RFID capture, from the ear tag to independent verification.
 *
 * The cryptography is real and runs in this browser with the same functions the verifier uses:
 * RFID hash, the 220-byte v2 envelope, a P-256 Station signature (fresh demo key per run), the
 * Lastro PDAs for the configured program, and the final independent checks. What is simulated is
 * transport and settlement: nothing is sent to the API, no wallet signs and no transaction is
 * submitted. Each step says which of the two it is.
 */
import { p256 } from '@noble/curves/nist.js'
import { computed, ref } from 'vue'
import { webConfig } from '../config'
import { locale, type Localized } from '../i18n'
import { fromHex, rfidHash, stationId, toHex } from '../protocol/hash'
import { isLowSP256Signature, verifyStationSignature } from '../protocol/p256'
import {
  encodeV2DomainEvent,
  V2EventType,
  v2EventHashOfBytes,
  v2IdentifierPayloadHash,
  V2_SCHEMA_VERSION,
} from '../protocol/v2/domainEvent'
import { assetPda, eventPda, rfidBindingPda } from '../solana/pda'

export type StepId = 'tag' | 'station' | 'agent' | 'api' | 'wallet' | 'chain' | 'verify'
export type StepState = 'idle' | 'running' | 'done' | 'failed' | 'skipped'

export type Scenario = 'normal' | 'offline' | 'tampered' | 'unknownStation' | 'alteredPackage'

export interface Detail {
  label: Localized
  value: string
  /** Long values (hashes, keys) are shown in monospace and can be copied. */
  mono?: boolean
}

export interface Step {
  id: StepId
  actor: Localized
  title: Localized
  /** Plain explanation of why the step exists. */
  why: Localized
  kind: 'computed' | 'simulated'
  state: StepState
  outcome: Localized | null
  details: Detail[]
}

export interface LogLine {
  at: string
  step: StepId
  tone: 'info' | 'ok' | 'warn' | 'error'
  text: Localized
}

const ZERO32 = new Uint8Array(32)

const definitions: Omit<Step, 'state' | 'outcome' | 'details'>[] = [
  {
    id: 'tag',
    actor: { pt: 'Brinco RFID', en: 'RFID ear tag' },
    title: { pt: 'Leitura do brinco', en: 'Tag read' },
    why: {
      pt: 'O número do brinco nunca vai para a rede em claro. O que circula é um hash com domínio próprio do Lastro.',
      en: 'The tag number never travels in clear. What moves is a hash under a Lastro-specific domain.',
    },
    kind: 'computed',
  },
  {
    id: 'station',
    actor: { pt: 'Estação (ESP32)', en: 'Station (ESP32)' },
    title: { pt: 'Estação monta e assina o evento', en: 'Station builds and signs the event' },
    why: {
      pt: 'A estação no curral assina os 220 bytes do evento com uma chave P-256 que não sai do chip. Quem altera um byte invalida a assinatura.',
      en: 'The Station at the chute signs the 220-byte event with a P-256 key that never leaves the chip. Changing one byte breaks the signature.',
    },
    kind: 'computed',
  },
  {
    id: 'agent',
    actor: { pt: 'Agent local', en: 'Local Agent' },
    title: { pt: 'Agent guarda na fila e envia', en: 'Agent queues and delivers' },
    why: {
      pt: 'Fazenda perde internet. O Agent grava o evento numa fila em disco (SQLite) e reenvia até a API confirmar; o hash do evento evita duplicidade.',
      en: 'Farms lose connectivity. The Agent writes the event to an on-disk queue (SQLite) and retries until the API acknowledges; the event hash prevents duplicates.',
    },
    kind: 'simulated',
  },
  {
    id: 'api',
    actor: { pt: 'API Lastro', en: 'Lastro API' },
    title: { pt: 'API confere a evidência', en: 'API checks the evidence' },
    why: {
      pt: 'Antes de aceitar, a API confere a assinatura, se a estação está cadastrada e se o evento ainda está dentro da validade.',
      en: 'Before accepting, the API checks the signature, that the Station is registered and that the event is still within its validity window.',
    },
    kind: 'computed',
  },
  {
    id: 'wallet',
    actor: { pt: 'Carteira do custodiante', en: 'Custodian wallet' },
    title: { pt: 'Custodiante autoriza a gravação', en: 'Custodian authorizes the write' },
    why: {
      pt: 'A API prepara a transação, mas só o dono atual do animal pode assiná-la. A carteira confere cada conta antes de assinar.',
      en: 'The API prepares the transaction, but only the current custodian can sign it. The wallet checks every account before signing.',
    },
    kind: 'simulated',
  },
  {
    id: 'chain',
    actor: { pt: 'Programa Lastro (Solana)', en: 'Lastro program (Solana)' },
    title: { pt: 'Solana grava identidade e vínculo', en: 'Solana records identity and binding' },
    why: {
      pt: 'O programa verifica a assinatura da estação de novo, on-chain, e cria as contas do animal, do brinco e do evento. Nada vale antes de finalizar.',
      en: 'The program re-checks the Station signature on-chain and creates the asset, tag and event accounts. Nothing counts until finalized.',
    },
    kind: 'simulated',
  },
  {
    id: 'verify',
    actor: { pt: 'Verificador independente', en: 'Independent verifier' },
    title: { pt: 'Qualquer pessoa confere', en: 'Anyone can check' },
    why: {
      pt: 'Comprador, frigorífico ou fiscal recalculam tudo a partir do pacote de evidências, sem confiar na API nem nesta tela.',
      en: 'A buyer, slaughterhouse or inspector recomputes everything from the evidence package, without trusting the API or this screen.',
    },
    kind: 'computed',
  },
]

function freshSteps(): Step[] {
  return definitions.map((step) => ({ ...step, state: 'idle', outcome: null, details: [] }))
}

function randomBytes(length: number): Uint8Array {
  const bytes = new Uint8Array(length)
  crypto.getRandomValues(bytes)
  return bytes
}

export function short(hex: string, keep = 8): string {
  return hex.length <= keep * 2 + 1 ? hex : `${hex.slice(0, keep)}…${hex.slice(-keep)}`
}

/** Random 15-digit ISO 11784 style tag number (076 = Brazil). */
export function randomTagNumber(): string {
  const digits = Array.from(randomBytes(12), (byte) => (byte % 10).toString()).join('')
  return `076${digits}`
}

/** The tag number as the 8 canonical bytes the protocol hashes. */
function canonicalTag(tagNumber: string): Uint8Array {
  const value = BigInt(tagNumber.replace(/\D/g, '') || '0')
  const bytes = new Uint8Array(8)
  new DataView(bytes.buffer).setBigUint64(0, value & 0xffff_ffff_ffff_ffffn, false)
  return bytes
}

interface Work {
  tagBytes: Uint8Array
  tagHash: Uint8Array
  stationSecret: Uint8Array
  stationPub: Uint8Array
  stationId: Uint8Array
  assetId: Uint8Array
  eventId: Uint8Array
  envelope: Uint8Array
  signature: Uint8Array
  eventHash: string
  received: Uint8Array
  anchoredEventHash: string
}

export function useSystemFlow() {
  const steps = ref<Step[]>(freshSteps())
  const log = ref<LogLine[]>([])
  const scenario = ref<Scenario>('normal')
  const tagNumber = ref(randomTagNumber())
  const cursor = ref(0)
  const running = ref(false)
  const finished = ref(false)
  const playing = ref(false)
  let work: Partial<Work> = {}
  let autoplay = false

  const current = computed(() => steps.value[cursor.value] ?? null)
  const verdict = computed<'valid' | 'invalid' | 'rejected' | null>(() => {
    if (!finished.value) return null
    const failed = steps.value.find((step) => step.state === 'failed')
    if (!failed) return 'valid'
    return failed.id === 'verify' ? 'invalid' : 'rejected'
  })

  function note(step: StepId, tone: LogLine['tone'], text: Localized): void {
    const at = new Date().toLocaleTimeString('pt-BR', { hour12: false })
    log.value = [...log.value, { at, step, tone, text }]
  }

  function patch(index: number, update: Partial<Step>): void {
    steps.value = steps.value.map((step, i) => (i === index ? { ...step, ...update } : step))
  }

  const deployment = () => fromHex(webConfig.lastroDeploymentId, 32)

  const run: Record<StepId, (index: number) => Promise<boolean>> = {
    async tag(index) {
      work.tagBytes = canonicalTag(tagNumber.value)
      work.tagHash = await rfidHash(work.tagBytes)
      patch(index, {
        outcome: { pt: 'Brinco lido e convertido em hash.', en: 'Tag read and hashed.' },
        details: [
          {
            label: { pt: 'Número do brinco', en: 'Tag number' },
            value: tagNumber.value,
            mono: true,
          },
          {
            label: { pt: 'Bytes canônicos', en: 'Canonical bytes' },
            value: toHex(work.tagBytes),
            mono: true,
          },
          {
            label: { pt: 'Hash do brinco', en: 'Tag hash' },
            value: toHex(work.tagHash),
            mono: true,
          },
        ],
      })
      note('tag', 'info', {
        pt: `Brinco ${tagNumber.value} lido`,
        en: `Tag ${tagNumber.value} read`,
      })
      return true
    },

    async station(index) {
      work.stationSecret = p256.utils.randomSecretKey()
      work.stationPub = p256.getPublicKey(work.stationSecret, true)
      work.stationId = await stationId(work.stationPub)
      work.assetId = randomBytes(32)
      work.eventId = randomBytes(32)
      const now = BigInt(Math.floor(Date.now() / 1000))
      work.envelope = encodeV2DomainEvent({
        schemaVersion: V2_SCHEMA_VERSION,
        eventType: V2EventType.IdentifierBound,
        deploymentId: webConfig.lastroDeploymentId,
        subjectId: toHex(work.assetId),
        eventId: toHex(work.eventId),
        stateVersion: 1n,
        expectedPreviousHash: toHex(ZERO32),
        payloadHash: await v2IdentifierPayloadHash(ZERO32, work.tagHash!),
        sourceId: toHex(work.stationId),
        observedAt: now,
        expiresAt: now + 3600n,
      })
      work.signature = p256.sign(work.envelope, work.stationSecret, { lowS: true })
      work.eventHash = await v2EventHashOfBytes(work.envelope)
      patch(index, {
        outcome: {
          pt: 'Evento de vínculo assinado pela estação.',
          en: 'Binding event signed by the Station.',
        },
        details: [
          { label: { pt: 'Tipo de evento', en: 'Event type' }, value: 'IDENTIFIER_BOUND' },
          { label: { pt: 'Envelope', en: 'Envelope' }, value: `${work.envelope.length} bytes` },
          {
            label: { pt: 'ID da estação', en: 'Station ID' },
            value: toHex(work.stationId),
            mono: true,
          },
          { label: { pt: 'ID do animal', en: 'Asset ID' }, value: toHex(work.assetId), mono: true },
          { label: { pt: 'Hash do evento', en: 'Event hash' }, value: work.eventHash, mono: true },
          {
            label: { pt: 'Assinatura P-256', en: 'P-256 signature' },
            value: toHex(work.signature),
            mono: true,
          },
        ],
      })
      note('station', 'ok', {
        pt: `Evento assinado (${short(work.eventHash, 6)})`,
        en: `Event signed (${short(work.eventHash, 6)})`,
      })
      return true
    },

    async agent(index) {
      const attempts = scenario.value === 'offline' ? 3 : 1
      for (let attempt = 1; attempt < attempts; attempt += 1) {
        note('agent', 'warn', {
          pt: `Tentativa ${attempt}: sem conexão com a API, evento continua na fila`,
          en: `Attempt ${attempt}: API unreachable, event stays queued`,
        })
        await wait(autoplay ? 700 : 350)
      }
      work.received = new Uint8Array(work.envelope!)
      if (scenario.value === 'tampered') {
        // Someone on the path changes the observation time by one second.
        work.received[204] = (work.received[204] ?? 0) ^ 0x01
        note('agent', 'warn', {
          pt: 'Um byte do envelope foi alterado no caminho até a API',
          en: 'One envelope byte was changed on the way to the API',
        })
      }
      patch(index, {
        outcome:
          attempts > 1
            ? {
                pt: `Entregue na ${attempts}ª tentativa, depois que a conexão voltou.`,
                en: `Delivered on attempt ${attempts}, once the link came back.`,
              }
            : {
                pt: 'Entregue à API na primeira tentativa.',
                en: 'Delivered to the API on the first attempt.',
              },
        details: [
          { label: { pt: 'Fila', en: 'Queue' }, value: 'outbox.sqlite' },
          {
            label: { pt: 'Chave de idempotência', en: 'Idempotency key' },
            value: work.eventHash!,
            mono: true,
          },
          { label: { pt: 'Tentativas', en: 'Attempts' }, value: String(attempts) },
        ],
      })
      note('agent', 'ok', { pt: 'Evento entregue à API', en: 'Event delivered to the API' })
      return true
    },

    async api(index) {
      const registered = scenario.value !== 'unknownStation'
      const signatureOk = verifyStationSignature(work.received!, work.stationPub!, work.signature!)
      const lowS = isLowSP256Signature(work.signature!)
      const view = new DataView(work.received!.buffer)
      const observedAt = view.getBigInt64(204, true)
      const expiresAt = view.getBigInt64(212, true)
      const now = BigInt(Math.floor(Date.now() / 1000))
      const fresh = observedAt <= now + 60n && now <= expiresAt
      const checks: Detail[] = [
        {
          label: { pt: 'Assinatura confere com os bytes', en: 'Signature matches the bytes' },
          value: yes(signatureOk),
        },
        {
          label: { pt: 'Assinatura canônica (low-S)', en: 'Canonical signature (low-S)' },
          value: yes(lowS),
        },
        { label: { pt: 'Estação cadastrada', en: 'Station registered' }, value: yes(registered) },
        { label: { pt: 'Dentro da validade', en: 'Within validity window' }, value: yes(fresh) },
      ]
      const accepted = signatureOk && lowS && registered && fresh
      patch(index, {
        outcome: accepted
          ? {
              pt: 'Evidência aceita. Captura liberada para o custodiante.',
              en: 'Evidence accepted. Capture handed to the custodian.',
            }
          : !signatureOk
            ? {
                pt: 'Recusada: os bytes não batem com a assinatura da estação.',
                en: 'Rejected: the bytes do not match the Station signature.',
              }
            : {
                pt: 'Recusada: a estação não está cadastrada neste deployment.',
                en: 'Rejected: the Station is not registered in this deployment.',
              },
        details: checks,
      })
      note(
        'api',
        accepted ? 'ok' : 'error',
        accepted
          ? { pt: 'Evidência aceita pela API', en: 'Evidence accepted by the API' }
          : {
              pt: 'API recusou a evidência; nada foi gravado',
              en: 'API rejected the evidence; nothing was recorded',
            },
      )
      return accepted
    },

    async wallet(index) {
      const programId = webConfig.lastroProgramId
      const [asset] = await assetPda(programId, deployment(), work.assetId!)
      const [binding] = await rfidBindingPda(programId, deployment(), work.tagHash!)
      const [event] = await eventPda(programId, deployment(), work.eventId!)
      patch(index, {
        outcome: {
          pt: 'Transação conferida e assinada pela carteira.',
          en: 'Transaction checked and signed by the wallet.',
        },
        details: [
          {
            label: { pt: 'Instrução 1', en: 'Instruction 1' },
            value: 'Secp256r1 verify (Station)',
          },
          {
            label: { pt: 'Instrução 2', en: 'Instruction 2' },
            value: 'lastro_v2::bind_identifier',
          },
          { label: { pt: 'Conta do animal', en: 'Asset account' }, value: asset, mono: true },
          {
            label: { pt: 'Conta do brinco', en: 'Tag binding account' },
            value: binding,
            mono: true,
          },
          { label: { pt: 'Conta do evento', en: 'Event account' }, value: event, mono: true },
        ],
      })
      note('wallet', 'ok', {
        pt: 'Custodiante assinou a transação',
        en: 'Custodian signed the transaction',
      })
      return true
    },

    async chain(index) {
      work.anchoredEventHash = work.eventHash!
      patch(index, {
        outcome: {
          pt: 'Gravado e finalizado. Brinco ativo para este animal.',
          en: 'Recorded and finalized. Tag active for this asset.',
        },
        details: [
          {
            label: { pt: 'AssetState', en: 'AssetState' },
            value: { pt: 'versão 1, custodiante atual', en: 'version 1, current custodian' }[
              locale.value
            ],
          },
          { label: { pt: 'RfidBinding', en: 'RfidBinding' }, value: 'ACTIVE' },
          {
            label: { pt: 'EventAnchor', en: 'EventAnchor' },
            value: work.anchoredEventHash,
            mono: true,
          },
          { label: { pt: 'Compromisso', en: 'Commitment' }, value: 'finalized' },
        ],
      })
      note('chain', 'ok', {
        pt: 'Transação finalizada na Solana',
        en: 'Transaction finalized on Solana',
      })
      return true
    },

    async verify(index) {
      const packageEnvelope = new Uint8Array(work.envelope!)
      if (scenario.value === 'alteredPackage') {
        // Someone edits the exported package: claims the tag was read one second earlier.
        packageEnvelope[204] = (packageEnvelope[204] ?? 0) ^ 0x01
      }
      const recomputedTag = toHex(await rfidHash(work.tagBytes!))
      const recomputedHash = await v2EventHashOfBytes(packageEnvelope)
      const signatureOk = verifyStationSignature(packageEnvelope, work.stationPub!, work.signature!)
      const anchored = recomputedHash === work.anchoredEventHash
      const tagOk = recomputedTag === toHex(work.tagHash!)
      const stationOk = toHex(await stationId(work.stationPub!)) === toHex(work.stationId!)
      const valid = signatureOk && anchored && tagOk && stationOk
      patch(index, {
        outcome: valid
          ? {
              pt: 'Válido. Tudo recalculado bate com o que está na Solana.',
              en: 'Valid. Everything recomputed matches Solana.',
            }
          : {
              pt: 'Inválido. O pacote não corresponde ao que foi gravado.',
              en: 'Invalid. The package does not match what was recorded.',
            },
        details: [
          {
            label: { pt: 'Hash do brinco recalculado', en: 'Tag hash recomputed' },
            value: yes(tagOk),
          },
          {
            label: { pt: 'Estação confere com a chave', en: 'Station matches its key' },
            value: yes(stationOk),
          },
          {
            label: { pt: 'Assinatura da estação', en: 'Station signature' },
            value: yes(signatureOk),
          },
          {
            label: {
              pt: 'Evento igual ao gravado na Solana',
              en: 'Event equals the Solana record',
            },
            value: yes(anchored),
          },
        ],
      })
      note(
        'verify',
        valid ? 'ok' : 'error',
        valid
          ? { pt: 'Verificação independente: VÁLIDO', en: 'Independent verification: VALID' }
          : { pt: 'Verificação independente: INVÁLIDO', en: 'Independent verification: INVALID' },
      )
      return valid
    },
  }

  async function step(): Promise<void> {
    if (running.value || finished.value) return
    const index = cursor.value
    const target = steps.value[index]
    if (!target) return
    running.value = true
    patch(index, { state: 'running' })
    await wait(autoplay ? 650 : 250)
    let ok = false
    try {
      ok = await run[target.id](index)
    } catch (error) {
      note(target.id, 'error', {
        pt: `Erro inesperado: ${(error as Error).message}`,
        en: `Unexpected error: ${(error as Error).message}`,
      })
    }
    patch(index, { state: ok ? 'done' : 'failed' })
    running.value = false
    if (!ok || index === steps.value.length - 1) {
      steps.value = steps.value.map((s) => (s.state === 'idle' ? { ...s, state: 'skipped' } : s))
      finished.value = true
      autoplay = false
      return
    }
    cursor.value = index + 1
  }

  async function play(): Promise<void> {
    if (finished.value) reset()
    autoplay = true
    playing.value = true
    while (autoplay && !finished.value) {
      await step()
      if (autoplay && !finished.value) await wait(450)
    }
    autoplay = false
    playing.value = false
  }

  function pause(): void {
    autoplay = false
    playing.value = false
  }

  function reset(options: { newTag?: boolean } = {}): void {
    autoplay = false
    playing.value = false
    steps.value = freshSteps()
    log.value = []
    cursor.value = 0
    finished.value = false
    running.value = false
    work = {}
    if (options.newTag) tagNumber.value = randomTagNumber()
  }

  return {
    steps,
    log,
    scenario,
    tagNumber,
    cursor,
    current,
    running,
    finished,
    verdict,
    playing,
    step,
    play,
    pause,
    reset,
  }
}

function yes(value: boolean): string {
  const pt = locale.value === 'pt'
  return value ? (pt ? 'sim' : 'yes') : pt ? 'não' : 'no'
}

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}
