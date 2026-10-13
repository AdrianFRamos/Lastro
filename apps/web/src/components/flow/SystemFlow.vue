<script setup lang="ts">
/**
 * Interactive walkthrough of one capture. The visitor picks a scenario, then runs the flow all at
 * once or one step at a time and opens any finished step to see what went in and what came out.
 */
import { computed, nextTick, ref, watch } from 'vue'
import { tr, useCopy } from '../../i18n'
import { short, useSystemFlow, type Scenario, type Step } from '../../system/flow'

const flow = useSystemFlow()
const { steps, log, scenario, tagNumber, cursor, running, finished, verdict, playing } = flow

const selectedId = ref<Step['id'] | null>(null)
const logRef = ref<HTMLElement | null>(null)
const copied = ref<string | null>(null)

const copy = useCopy({
  pt: {
    scenario: 'Cenário',
    tag: 'Brinco',
    newTag: 'Outro brinco',
    run: 'Executar fluxo',
    pause: 'Pausar',
    next: 'Próxima etapa',
    reset: 'Recomeçar',
    computed: 'Calculado aqui no navegador',
    simulated: 'Simulado',
    why: 'Por que existe',
    waiting: 'Aguardando a etapa anterior.',
    notReached: 'Não executada: o fluxo parou antes.',
    log: 'Registro',
    logEmpty: 'Nada executado ainda. Use "Executar fluxo" ou avance uma etapa por vez.',
    copy: 'Copiar',
    copied: 'Copiado',
    stepOf: (n: number, total: number) => `Etapa ${n} de ${total}`,
    valid: 'Resultado: válido',
    validText:
      'O vínculo entre o brinco e o animal foi gravado e qualquer pessoa consegue conferir.',
    rejected: 'Resultado: recusado',
    rejectedText: (where: string) => `O fluxo parou em "${where}". Nada falso chegou à Solana.`,
    invalid: 'Resultado: inválido',
    invalidText:
      'O pacote apresentado não bate com o que está gravado. O verificador acusa a alteração.',
    note: 'Nada aqui é enviado à API nem à Solana. Hashes, assinatura e endereços são reais; transporte e gravação são simulados.',
    scenarios: {
      normal: 'Tudo certo',
      offline: 'Fazenda sem internet',
      tampered: 'Envelope alterado no caminho',
      unknownStation: 'Estação não cadastrada',
      alteredPackage: 'Pacote alterado depois',
    } satisfies Record<Scenario, string>,
    states: {
      idle: 'pendente',
      running: 'executando',
      done: 'ok',
      failed: 'falhou',
      skipped: 'não executada',
    },
  },
  en: {
    scenario: 'Scenario',
    tag: 'Tag',
    newTag: 'Another tag',
    run: 'Run flow',
    pause: 'Pause',
    next: 'Next step',
    reset: 'Start over',
    computed: 'Computed here in the browser',
    simulated: 'Simulated',
    why: 'Why it exists',
    waiting: 'Waiting for the previous step.',
    notReached: 'Not run: the flow stopped earlier.',
    log: 'Log',
    logEmpty: 'Nothing has run yet. Use "Run flow" or advance one step at a time.',
    copy: 'Copy',
    copied: 'Copied',
    stepOf: (n: number, total: number) => `Step ${n} of ${total}`,
    valid: 'Result: valid',
    validText: 'The link between tag and animal was recorded and anyone can check it.',
    rejected: 'Result: rejected',
    rejectedText: (where: string) =>
      `The flow stopped at "${where}". Nothing false reached Solana.`,
    invalid: 'Result: invalid',
    invalidText:
      'The package presented does not match what was recorded. The verifier flags the change.',
    note: 'Nothing here is sent to the API or to Solana. Hashes, signature and addresses are real; transport and settlement are simulated.',
    scenarios: {
      normal: 'Everything fine',
      offline: 'Farm without internet',
      tampered: 'Envelope changed in transit',
      unknownStation: 'Unregistered Station',
      alteredPackage: 'Package edited afterwards',
    } satisfies Record<Scenario, string>,
    states: {
      idle: 'pending',
      running: 'running',
      done: 'ok',
      failed: 'failed',
      skipped: 'not run',
    },
  },
})

const scenarioOrder: Scenario[] = [
  'normal',
  'offline',
  'tampered',
  'unknownStation',
  'alteredPackage',
]

const selected = computed<Step | null>(() => {
  const id = selectedId.value
  return steps.value.find((step) => step.id === id) ?? steps.value[cursor.value] ?? null
})
const selectedIndex = computed(() =>
  selected.value ? steps.value.findIndex((step) => step.id === selected.value!.id) : 0,
)
const failedStep = computed(() => steps.value.find((step) => step.state === 'failed') ?? null)
const started = computed(() => log.value.length > 0 || running.value)

// While the flow runs, the panel follows it; clicking a step pins the panel to that step.
watch(cursor, () => {
  if (playing.value || running.value) selectedId.value = null
})
watch(
  () => log.value.length,
  async () => {
    await nextTick()
    logRef.value?.scrollTo({ top: logRef.value.scrollHeight })
  },
)
watch(scenario, () => flow.reset())

function select(step: Step): void {
  selectedId.value = step.id
}

function newTag(): void {
  flow.reset({ newTag: true })
}

async function copyValue(value: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(value)
    copied.value = value
    setTimeout(() => (copied.value = null), 1400)
  } catch {
    // Clipboard can be blocked; the value is still selectable on screen.
  }
}
</script>

<template>
  <div class="flow" :data-finished="finished">
    <div class="flow__toolbar">
      <fieldset class="flow__scenarios" :disabled="running || playing">
        <legend>{{ copy.scenario }}</legend>
        <label v-for="option in scenarioOrder" :key="option" :data-checked="scenario === option">
          <input v-model="scenario" type="radio" name="flow-scenario" :value="option" />
          {{ copy.scenarios[option] }}
        </label>
      </fieldset>

      <div class="flow__controls">
        <div class="flow__tag">
          <span>{{ copy.tag }}</span>
          <code>{{ tagNumber }}</code>
          <button type="button" class="flow__link" :disabled="running || playing" @click="newTag">
            {{ copy.newTag }}
          </button>
        </div>
        <div class="flow__buttons">
          <button
            v-if="!playing"
            type="button"
            class="btn btn--primary"
            data-action="run-flow"
            :disabled="running"
            @click="flow.play()"
          >
            {{ copy.run }}
          </button>
          <button v-else type="button" class="btn btn--primary" @click="flow.pause()">
            {{ copy.pause }}
          </button>
          <button
            type="button"
            class="btn"
            data-action="next-step"
            :disabled="running || playing || finished"
            @click="flow.step()"
          >
            {{ copy.next }}
          </button>
          <button
            type="button"
            class="btn btn--ghost"
            :disabled="!started || running"
            @click="flow.reset()"
          >
            {{ copy.reset }}
          </button>
        </div>
      </div>
    </div>

    <div class="flow__body">
      <ol class="flow__steps">
        <li v-for="(step, index) in steps" :key="step.id">
          <button
            type="button"
            class="flow-step"
            :data-state="step.state"
            :aria-current="selected?.id === step.id ? 'step' : undefined"
            @click="select(step)"
          >
            <span class="flow-step__marker" aria-hidden="true">{{ index + 1 }}</span>
            <span class="flow-step__text">
              <span class="flow-step__actor">{{ tr(step.actor) }}</span>
              <span class="flow-step__title">{{ tr(step.title) }}</span>
            </span>
            <span class="flow-step__state">{{ copy.states[step.state] }}</span>
          </button>
        </li>
      </ol>

      <section v-if="selected" class="flow__detail" aria-live="polite">
        <header class="flow__detail-head">
          <p>{{ copy.stepOf(selectedIndex + 1, steps.length) }} · {{ tr(selected.actor) }}</p>
          <span class="flow__kind" :data-kind="selected.kind">
            {{ selected.kind === 'computed' ? copy.computed : copy.simulated }}
          </span>
        </header>
        <h3>{{ tr(selected.title) }}</h3>
        <p class="flow__why">
          <strong>{{ copy.why }}:</strong> {{ tr(selected.why) }}
        </p>

        <p v-if="selected.outcome" class="flow__outcome" :data-state="selected.state">
          {{ tr(selected.outcome) }}
        </p>
        <p v-else-if="selected.state === 'skipped'" class="flow__pending">{{ copy.notReached }}</p>
        <p v-else-if="selected.state === 'running'" class="flow__pending">
          {{ copy.states.running }}…
        </p>
        <p v-else class="flow__pending">{{ copy.waiting }}</p>

        <dl v-if="selected.details.length" class="flow__details">
          <template v-for="detail in selected.details" :key="tr(detail.label)">
            <dt>{{ tr(detail.label) }}</dt>
            <dd>
              <template v-if="detail.mono">
                <code :title="detail.value">{{ short(detail.value, 10) }}</code>
                <button type="button" class="flow__copy" @click="copyValue(detail.value)">
                  {{ copied === detail.value ? copy.copied : copy.copy }}
                </button>
              </template>
              <span v-else :data-answer="detail.value">{{ detail.value }}</span>
            </dd>
          </template>
        </dl>
      </section>
    </div>

    <div v-if="verdict" class="flow__verdict" :data-verdict="verdict" role="status">
      <strong>{{
        verdict === 'valid' ? copy.valid : verdict === 'invalid' ? copy.invalid : copy.rejected
      }}</strong>
      <span v-if="verdict === 'valid'">{{ copy.validText }}</span>
      <span v-else-if="verdict === 'invalid'">{{ copy.invalidText }}</span>
      <span v-else>{{ copy.rejectedText(failedStep ? tr(failedStep.title) : '') }}</span>
    </div>

    <div class="flow__log">
      <h4>{{ copy.log }}</h4>
      <ol v-if="log.length" ref="logRef">
        <li v-for="(line, index) in log" :key="index" :data-tone="line.tone">
          <time>{{ line.at }}</time>
          <span class="flow__log-step">{{ line.step }}</span>
          <span>{{ tr(line.text) }}</span>
        </li>
      </ol>
      <p v-else>{{ copy.logEmpty }}</p>
    </div>

    <p class="flow__note">{{ copy.note }}</p>
  </div>
</template>

<style scoped>
.flow {
  display: grid;
  gap: 16px;
}

.flow__toolbar {
  display: grid;
  gap: 14px;
  padding: 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
}

.flow__scenarios {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 0;
  padding: 0;
  border: 0;
}

.flow__scenarios legend {
  float: left;
  margin-right: 6px;
  padding: 7px 0;
  color: var(--muted);
  font-size: 13px;
}

.flow__scenarios label {
  display: inline-flex;
  align-items: center;
  padding: 6px 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--canvas);
  color: var(--text);
  font-size: 13px;
  cursor: pointer;
}

.flow__scenarios label[data-checked='true'] {
  border-color: var(--primary);
  background: var(--primary-soft);
  color: var(--primary-hover);
  font-weight: 600;
}

.flow__scenarios label {
  position: relative;
}

.flow__scenarios input {
  position: absolute;
  width: 1px;
  height: 1px;
  min-height: 0;
  margin: 0;
  padding: 0;
  overflow: hidden;
  clip-path: inset(50%);
  opacity: 0;
}

.flow__scenarios label:has(input:focus-visible) {
  outline: 2px solid var(--chain);
  outline-offset: 2px;
}

.flow__scenarios:disabled label {
  cursor: not-allowed;
  opacity: 0.6;
}

.flow__controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.flow__tag {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  color: var(--muted);
  font-size: 13px;
}

.flow__tag code {
  padding: 3px 8px;
  border-radius: var(--radius-sm);
  background: var(--raised);
  color: var(--text);
  font-size: 13px;
}

.flow__link {
  padding: 0;
  border: 0;
  background: none;
  color: var(--primary);
  font-size: 13px;
  text-decoration: underline;
  text-underline-offset: 3px;
  cursor: pointer;
}

.flow__link:disabled {
  color: var(--faint);
  cursor: not-allowed;
}

.flow__buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.btn {
  min-height: 38px;
  padding: 8px 14px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--text);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}

.btn:hover:not(:disabled) {
  background: var(--raised);
}

.btn--primary {
  border-color: var(--primary);
  background: var(--primary);
  color: var(--on-primary);
}

.btn--primary:hover:not(:disabled) {
  border-color: var(--primary-hover);
  background: var(--primary-hover);
}

.btn--ghost {
  border-color: transparent;
  background: transparent;
  color: var(--muted);
}

.btn:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.flow__body {
  display: grid;
  grid-template-columns: minmax(260px, 0.8fr) minmax(0, 1.4fr);
  gap: 16px;
  align-items: start;
}

.flow__steps {
  position: relative;
  display: grid;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.flow-step {
  display: grid;
  grid-template-columns: 30px minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--text);
  text-align: left;
  cursor: pointer;
  transition:
    border-color 150ms ease,
    background 150ms ease;
}

.flow-step:hover {
  border-color: var(--border-strong);
}

.flow-step[aria-current='step'] {
  border-color: var(--text);
  box-shadow: var(--shadow-sm);
}

.flow-step__marker {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border: 1px solid var(--border-strong);
  border-radius: 50%;
  background: var(--canvas);
  color: var(--muted);
  font-size: 13px;
  font-weight: 700;
}

.flow-step__text {
  display: grid;
  gap: 1px;
  min-width: 0;
}

.flow-step__actor {
  color: var(--muted);
  font-size: 12px;
}

.flow-step__title {
  overflow: hidden;
  font-size: 14px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.flow-step__state {
  color: var(--faint);
  font-size: 12px;
}

.flow-step[data-state='running'] {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.flow-step[data-state='running'] .flow-step__marker {
  border-color: var(--accent);
  background: var(--accent);
  color: var(--on-primary);
}

.flow-step[data-state='running'] .flow-step__state {
  color: var(--accent);
  font-weight: 600;
}

.flow-step[data-state='done'] .flow-step__marker {
  border-color: var(--primary);
  background: var(--primary);
  color: var(--on-primary);
}

.flow-step[data-state='done'] .flow-step__state {
  color: var(--primary);
  font-weight: 600;
}

.flow-step[data-state='failed'] {
  border-color: var(--danger);
  background: var(--danger-soft);
}

.flow-step[data-state='failed'] .flow-step__marker {
  border-color: var(--danger);
  background: var(--danger);
  color: var(--on-primary);
}

.flow-step[data-state='failed'] .flow-step__state {
  color: var(--danger);
  font-weight: 600;
}

.flow-step[data-state='skipped'] {
  border-style: dashed;
  background: transparent;
  color: var(--faint);
}

.flow__detail {
  min-width: 0;
  padding: 20px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
}

.flow__detail-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.flow__detail-head p {
  margin: 0;
  color: var(--muted);
  font-size: 13px;
}

.flow__kind {
  padding: 3px 9px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
}

.flow__kind[data-kind='computed'] {
  background: var(--proof-soft);
  color: var(--proof);
}

.flow__kind[data-kind='simulated'] {
  background: var(--raised);
  color: var(--muted);
}

.flow__detail h3 {
  margin: 10px 0 8px;
  font-size: 20px;
  line-height: 1.25;
}

.flow__why {
  margin: 0;
  color: var(--muted);
  font-size: 14px;
  line-height: 1.55;
}

.flow__why strong {
  color: var(--text);
  font-weight: 600;
}

.flow__outcome,
.flow__pending {
  margin: 16px 0 0;
  padding: 10px 12px;
  border-radius: var(--radius-md);
  font-size: 14px;
}

.flow__outcome {
  border-left: 3px solid var(--primary);
  background: var(--primary-soft);
}

.flow__outcome[data-state='failed'] {
  border-left-color: var(--danger);
  background: var(--danger-soft);
}

.flow__pending {
  background: var(--canvas);
  color: var(--muted);
}

.flow__details {
  display: grid;
  grid-template-columns: minmax(150px, max-content) minmax(0, 1fr);
  gap: 8px 16px;
  margin: 16px 0 0;
  font-size: 13px;
}

.flow__details dt {
  color: var(--muted);
}

.flow__details dd {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  min-width: 0;
  margin: 0;
}

.flow__details code {
  overflow-wrap: anywhere;
  font-size: 12.5px;
}

.flow__details [data-answer='sim'],
.flow__details [data-answer='yes'] {
  color: var(--success);
  font-weight: 600;
}

.flow__details [data-answer='não'],
.flow__details [data-answer='no'] {
  color: var(--danger);
  font-weight: 600;
}

.flow__copy {
  padding: 1px 7px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--canvas);
  color: var(--muted);
  font-size: 11px;
  cursor: pointer;
}

.flow__verdict {
  display: grid;
  gap: 2px;
  padding: 14px 16px;
  border: 1px solid var(--success);
  border-radius: var(--radius-lg);
  background: var(--success-soft);
  font-size: 14px;
}

.flow__verdict strong {
  font-size: 15px;
}

.flow__verdict[data-verdict='rejected'] {
  border-color: var(--warning);
  background: var(--warning-soft);
}

.flow__verdict[data-verdict='invalid'] {
  border-color: var(--danger);
  background: var(--danger-soft);
}

.flow__log {
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
}

.flow__log h4 {
  margin: 0;
  padding: 10px 16px;
  border-bottom: 1px solid var(--border);
  color: var(--muted);
  font-size: 13px;
  font-weight: 600;
}

.flow__log ol {
  max-height: 180px;
  margin: 0;
  padding: 8px 0;
  overflow-y: auto;
  list-style: none;
  font: 12.5px/1.6 var(--font-mono);
}

.flow__log li {
  display: grid;
  grid-template-columns: 70px 64px minmax(0, 1fr);
  gap: 8px;
  padding: 1px 16px;
}

.flow__log time {
  color: var(--faint);
}

.flow__log-step {
  color: var(--muted);
}

.flow__log li[data-tone='ok'] span:last-child {
  color: var(--success);
}

.flow__log li[data-tone='warn'] span:last-child {
  color: var(--warning);
}

.flow__log li[data-tone='error'] span:last-child {
  color: var(--danger);
}

.flow__log p {
  margin: 0;
  padding: 14px 16px;
  color: var(--muted);
  font-size: 13px;
}

.flow__note {
  margin: 0;
  color: var(--faint);
  font-size: 12.5px;
}

@media (max-width: 860px) {
  .flow__body {
    grid-template-columns: minmax(0, 1fr);
  }

  .flow__details {
    grid-template-columns: minmax(0, 1fr);
    gap: 2px;
  }

  .flow__details dd {
    margin-bottom: 8px;
  }

  .flow__log li {
    grid-template-columns: 62px minmax(0, 1fr);
  }

  .flow__log-step {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .flow-step {
    transition: none;
  }
}
</style>
