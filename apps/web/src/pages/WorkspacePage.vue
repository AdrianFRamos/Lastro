<script setup lang="ts">
/**
 * Workspace reached after the demo login. Each chain participant manages its own records
 * (create, read, update, delete); the common user browses every participant's records without
 * changing them. Simulated: records stay in this browser and nothing reaches the API or chain.
 */
import { computed, ref, watch } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import ChainHeader from '../components/chain/ChainHeader.vue'
import ResourceManager from '../components/workspace/ResourceManager.vue'
import { routes } from '../demo/chainHistory'
import { findRole, participants, resourcesOf, type ParticipantId } from '../demo/roles'
import { resetRecords, session, signOut } from '../demo/workspace'
import { tr, useCopy } from '../i18n'

const router = useRouter()

const copy = useCopy({
  en: {
    kicker: 'WORKSPACE',
    signedInAs: 'Signed in as',
    signOut: 'Sign out',
    full: 'Full access: create, view, edit and delete',
    read: 'Read only: view the records of the whole chain',
    simulated: 'Simulated: records stay in this browser',
    participant: 'Chain participant',
    sections: 'Sections',
    reset: 'Restore example data',
    resetDone: 'Example data restored.',
    history: 'See the chain history',
  },
  pt: {
    kicker: 'PAINEL',
    signedInAs: 'Conectado como',
    signOut: 'Sair',
    full: 'Acesso completo: cadastrar, ver, editar e excluir',
    read: 'Somente leitura: consulta os registros de toda a cadeia',
    simulated: 'Simulação: os registros ficam neste navegador',
    participant: 'Elo da cadeia',
    sections: 'Seções',
    reset: 'Restaurar dados de exemplo',
    resetDone: 'Dados de exemplo restaurados.',
    history: 'Ver o histórico da cadeia',
  },
})

const role = computed(() => findRole(session.value?.role))
const readOnly = computed(() => role.value?.access !== 'crud')

/** Whose records are shown: the participant itself, or the one the common user picked. */
const viewedParticipant = ref<ParticipantId>('producer')
const participant = computed<ParticipantId>(() =>
  role.value && role.value.id !== 'viewer' ? role.value.id : viewedParticipant.value,
)
const sections = computed(() => resourcesOf(participant.value))
const activeId = ref('')
const active = computed(
  () => sections.value.find((resource) => resource.id === activeId.value) ?? sections.value[0],
)
const resetStatus = ref('')

watch(participant, () => (activeId.value = ''))

// Signed out here or in another tab: back to the login.
watch(
  session,
  (current) => {
    if (!current) void router.replace({ name: 'login' })
  },
  { immediate: true },
)

function leave(): void {
  signOut()
}

function restore(): void {
  resetRecords()
  resetStatus.value = copy.value.resetDone
}
</script>

<template>
  <main v-if="role && session" class="workspace-page">
    <div class="workspace-page__frame">
      <ChainHeader>
        <template #default>
          <span class="workspace-page__who">
            <span class="sr-only">{{ copy.signedInAs }}</span>
            {{ tr(role.label) }}
          </span>
          <button type="button" class="chain-header__button" data-action="sign-out" @click="leave">
            {{ copy.signOut }}
          </button>
        </template>
      </ChainHeader>

      <section class="workspace-hero" aria-labelledby="workspace-title">
        <p class="workspace-hero__kicker">{{ copy.kicker }} · {{ tr(role.label).toUpperCase() }}</p>
        <h1 id="workspace-title">{{ tr(role.label) }}</h1>
        <p class="workspace-hero__summary">{{ tr(role.summary) }}</p>
        <div class="workspace-hero__meta">
          <span :class="readOnly ? 'is-read' : 'is-full'">{{
            readOnly ? copy.read : copy.full
          }}</span>
          <span>{{ session.email }}</span>
          <span class="is-muted">{{ copy.simulated }}</span>
        </div>
      </section>

      <nav
        v-if="role.id === 'viewer'"
        class="workspace-tabs workspace-tabs--participants"
        :aria-label="copy.participant"
      >
        <button
          v-for="candidate in participants"
          :key="candidate.id"
          type="button"
          :aria-pressed="participant === candidate.id"
          :data-participant="candidate.id"
          @click="viewedParticipant = candidate.id"
        >
          {{ tr(candidate.label) }}
        </button>
      </nav>

      <nav class="workspace-tabs" :aria-label="copy.sections">
        <button
          v-for="resource in sections"
          :key="resource.id"
          type="button"
          :aria-pressed="active?.id === resource.id"
          :data-resource="resource.id"
          @click="activeId = resource.id"
        >
          {{ tr(resource.label) }}
        </button>
      </nav>

      <ResourceManager v-if="active" :key="active.id" :resource="active" :read-only="readOnly" />

      <footer class="workspace-page__footer">
        <RouterLink :to="routes.history">← {{ copy.history }}</RouterLink>
        <button v-if="!readOnly" type="button" data-action="reset" @click="restore">
          {{ copy.reset }}
        </button>
        <span v-if="resetStatus" role="status">{{ resetStatus }}</span>
      </footer>
    </div>
  </main>
</template>

<style scoped>
.workspace-page {
  min-height: 100svh;
  background:
    radial-gradient(
      circle at 50% 0%,
      color-mix(in srgb, var(--primary) 14%, transparent),
      transparent 42%
    ),
    var(--canvas);
  color: var(--text);
}

.workspace-page__frame {
  display: grid;
  gap: 28px;
  width: min(100% - 64px, 1400px);
  margin: 0 auto;
  padding: 24px 0 64px;
}

.workspace-page__who {
  padding: 10px 14px;
  border: 1px solid color-mix(in srgb, var(--proof) 45%, var(--border));
  border-radius: 999px;
  color: var(--proof);
  font: 700 12px/1 var(--font-mono);
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.workspace-hero {
  display: grid;
  gap: 10px;
  padding: 24px 0 8px;
}

.workspace-hero__kicker {
  margin: 0;
  color: var(--proof);
  font: 700 12px/1 var(--font-mono);
  letter-spacing: 0.18em;
}

.workspace-hero h1 {
  margin: 0;
  font-size: clamp(36px, 5vw, 56px);
  letter-spacing: -0.04em;
  line-height: 1;
}

.workspace-hero__summary {
  max-width: 640px;
  margin: 0;
  color: var(--muted);
  font-size: 17px;
  line-height: 1.5;
}

.workspace-hero__meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 6px;
}

.workspace-hero__meta span {
  padding: 7px 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  font: 700 12px/1 var(--font-mono);
}

.workspace-hero__meta .is-full {
  border-color: color-mix(in srgb, var(--proof) 45%, var(--border));
  color: var(--proof);
}

.workspace-hero__meta .is-read {
  border-color: color-mix(in srgb, var(--chain) 45%, var(--border));
  color: var(--chain);
}

.workspace-hero__meta .is-muted {
  color: var(--muted);
}

.workspace-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.workspace-tabs button {
  padding: 11px 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--muted);
  font: 700 13px/1 var(--font-sans);
  cursor: pointer;
}

.workspace-tabs button:hover,
.workspace-tabs button:focus-visible {
  border-color: var(--border-strong);
  color: var(--text);
}

.workspace-tabs button[aria-pressed='true'] {
  border-color: color-mix(in srgb, var(--primary) 78%, var(--border));
  background: color-mix(in srgb, var(--primary) 22%, var(--canvas));
  color: var(--text);
}

.workspace-tabs--participants button[aria-pressed='true'] {
  border-color: color-mix(in srgb, var(--proof) 60%, var(--border));
  background: color-mix(in srgb, var(--proof) 14%, var(--canvas));
}

.workspace-page__footer {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 16px;
  color: var(--muted);
  font: 700 12px/1 var(--font-mono);
}

.workspace-page__footer a {
  color: var(--proof);
  text-decoration: none;
}

.workspace-page__footer button {
  padding: 0;
  border: 0;
  background: none;
  color: var(--muted);
  font: inherit;
  text-decoration: underline;
  cursor: pointer;
}

@media (max-width: 768px) {
  .workspace-page__frame {
    width: calc(100% - 32px);
    padding-top: 16px;
  }

  .workspace-page__who {
    display: none;
  }
}
</style>
