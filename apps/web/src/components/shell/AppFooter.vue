<script setup lang="ts">
/** Global footer: map of the system areas and a live status bar for the connected services. */
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { webConfig } from '../../config'
import { useCopy } from '../../i18n'
import { clusterName, useSystemStatus } from '../../system/status'

const { api, rpc, slot } = useSystemStatus()

const copy = useCopy({
  pt: {
    about:
      'Identidade e custódia do rebanho registradas com evidência física e conferíveis por qualquer pessoa.',
    operate: 'Operar',
    trace: 'Rastrear',
    project: 'Projeto',
    console: 'Console de operação',
    operations: 'Operações autenticadas',
    workspace: 'Painel do participante',
    journey: 'Jornada do lote',
    lineage: 'Linhagem pública',
    verify: 'Verificador independente',
    how: 'Como funciona',
    why: 'Por que o Lastro',
    next: 'Próximos passos',
    api: 'API',
    chain: 'Solana',
    program: 'Programa',
    slot: 'slot',
    online: 'no ar',
    offline: 'fora do ar',
    checking: 'verificando',
  },
  en: {
    about: 'Herd identity and custody recorded with physical evidence, checkable by anyone.',
    operate: 'Operate',
    trace: 'Trace',
    project: 'Project',
    console: 'Operator console',
    operations: 'Authenticated operations',
    workspace: 'Participant workspace',
    journey: 'Lot journey',
    lineage: 'Public lineage',
    verify: 'Independent verifier',
    how: 'How it works',
    why: 'Why Lastro',
    next: 'Next steps',
    api: 'API',
    chain: 'Solana',
    program: 'Program',
    slot: 'slot',
    online: 'up',
    offline: 'down',
    checking: 'checking',
  },
})

const program = computed(() => {
  const id = webConfig.lastroProgramId
  return id.length > 12 ? `${id.slice(0, 4)}…${id.slice(-4)}` : id
})
const year = new Date().getFullYear()
</script>

<template>
  <footer class="site-footer">
    <div class="site-footer__inner">
      <div class="site-footer__about">
        <RouterLink class="site-footer__brand" to="/">
          <img src="/logo.svg" alt="" width="24" height="24" />
          <span>Lastro</span>
        </RouterLink>
        <p>{{ copy.about }}</p>
      </div>

      <nav class="site-footer__col" :aria-label="copy.operate">
        <h2>{{ copy.operate }}</h2>
        <RouterLink to="/demo">{{ copy.console }}</RouterLink>
        <RouterLink to="/operations">{{ copy.operations }}</RouterLink>
        <RouterLink to="/painel">{{ copy.workspace }}</RouterLink>
      </nav>
      <nav class="site-footer__col" :aria-label="copy.trace">
        <h2>{{ copy.trace }}</h2>
        <RouterLink to="/chain-history">{{ copy.journey }}</RouterLink>
        <RouterLink to="/lineage">{{ copy.lineage }}</RouterLink>
        <RouterLink to="/verify">{{ copy.verify }}</RouterLink>
      </nav>
      <nav class="site-footer__col" :aria-label="copy.project">
        <h2>{{ copy.project }}</h2>
        <RouterLink to="/#fluxo">{{ copy.how }}</RouterLink>
        <RouterLink to="/problem">{{ copy.why }}</RouterLink>
        <RouterLink to="/future">{{ copy.next }}</RouterLink>
      </nav>
    </div>

    <div class="site-footer__bar">
      <div class="site-footer__bar-inner">
        <ul class="site-footer__status" aria-live="polite">
          <li :data-state="api.state">
            <span class="dot" aria-hidden="true" />
            {{ copy.api }} {{ copy[api.state] }}
            <span v-if="api.latencyMs !== null" class="faint">{{ api.latencyMs }} ms</span>
          </li>
          <li :data-state="rpc.state">
            <span class="dot" aria-hidden="true" />
            {{ copy.chain }} {{ clusterName() }}
            <span v-if="slot !== null" class="faint"
              >{{ copy.slot }} {{ slot.toLocaleString() }}</span
            >
          </li>
          <li>
            {{ copy.program }} <code :title="webConfig.lastroProgramId">{{ program }}</code>
          </li>
        </ul>
        <span class="faint">© {{ year }} Lastro · protocolo v2</span>
      </div>
    </div>
  </footer>
</template>

<style scoped>
.site-footer {
  margin-top: auto;
  border-top: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
}

.site-footer__inner {
  display: grid;
  grid-template-columns: minmax(0, 1.6fr) repeat(3, minmax(0, 1fr));
  gap: 32px;
  width: min(100% - 48px, var(--page-width));
  margin: 0 auto;
  padding: 40px 0 32px;
}

.site-footer__brand {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: var(--text);
  font-size: 16px;
  font-weight: 700;
  text-decoration: none;
}

.site-footer__about p {
  max-width: 34ch;
  margin: 12px 0 0;
  color: var(--muted);
  font-size: 14px;
  line-height: 1.55;
}

.site-footer__col {
  display: grid;
  align-content: start;
  gap: 8px;
}

.site-footer__col h2 {
  margin: 0 0 4px;
  color: var(--text);
  font-size: 13px;
  font-weight: 700;
}

.site-footer__col a {
  color: var(--muted);
  font-size: 14px;
  text-decoration: none;
}

.site-footer__col a:hover {
  color: var(--primary);
  text-decoration: underline;
  text-underline-offset: 3px;
}

.site-footer__bar {
  border-top: 1px solid var(--border);
  background: var(--canvas);
}

.site-footer__bar-inner {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px 24px;
  width: min(100% - 48px, var(--page-width));
  margin: 0 auto;
  padding: 12px 0;
  font-size: 12.5px;
}

.site-footer__status {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 20px;
  margin: 0;
  padding: 0;
  list-style: none;
  color: var(--muted);
}

.site-footer__status li {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--faint);
}

[data-state='online'] .dot {
  background: var(--success);
}

[data-state='offline'] .dot {
  background: var(--danger);
}

.faint {
  color: var(--faint);
}

code {
  font-size: 12px;
}

@media (max-width: 860px) {
  .site-footer__inner {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    width: min(100% - 32px, var(--page-width));
  }

  .site-footer__about {
    grid-column: 1 / -1;
  }

  .site-footer__bar-inner {
    width: min(100% - 32px, var(--page-width));
  }
}

@media (max-width: 520px) {
  .site-footer__inner {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 24px;
  }
}
</style>
