<script setup lang="ts">
/**
 * Global header: brand, primary navigation, live service status, language and session.
 * Every page renders inside it, so pages no longer carry their own top bars.
 */
import { computed, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import LanguageSwitch from '../LanguageSwitch.vue'
import { findRole } from '../../demo/roles'
import { session, signOut } from '../../demo/workspace'
import { tr, useCopy } from '../../i18n'
import { useSystemStatus } from '../../system/status'

const route = useRoute()
const router = useRouter()
const { api, rpc } = useSystemStatus()
const menuOpen = ref(false)

const copy = useCopy({
  pt: {
    home: 'Início',
    how: 'Como funciona',
    journey: 'Jornada do lote',
    verify: 'Verificar',
    console: 'Console',
    operations: 'Operações',
    signIn: 'Entrar',
    workspace: 'Painel',
    signOut: 'Sair',
    visitor: 'Visitante',
    menu: 'Menu',
    nav: 'Navegação principal',
    online: 'Sistema no ar',
    partial: 'Instável',
    offline: 'Fora do ar',
    checking: 'Verificando',
    statusTitle: (a: string, r: string) => `API: ${a} · Solana: ${r}`,
  },
  en: {
    home: 'Home',
    how: 'How it works',
    journey: 'Lot journey',
    verify: 'Verify',
    console: 'Console',
    operations: 'Operations',
    signIn: 'Sign in',
    workspace: 'Workspace',
    signOut: 'Sign out',
    visitor: 'Visitor',
    menu: 'Menu',
    nav: 'Main navigation',
    online: 'System up',
    partial: 'Degraded',
    offline: 'Down',
    checking: 'Checking',
    statusTitle: (a: string, r: string) => `API: ${a} · Solana: ${r}`,
  },
})

const links = computed(() => [
  {
    to: '/',
    label: copy.value.home,
    match: (path: string, hash: string) => path === '/' && hash !== '#fluxo',
  },
  {
    to: '/#fluxo',
    label: copy.value.how,
    match: (path: string, hash: string) => path === '/' && hash === '#fluxo',
  },
  {
    to: '/chain-history',
    label: copy.value.journey,
    match: (path: string) => path.startsWith('/chain-history'),
  },
  {
    to: '/verify',
    label: copy.value.verify,
    match: (path: string) => path.startsWith('/verify') || path.startsWith('/lineage'),
  },
  { to: '/demo', label: copy.value.console, match: (path: string) => path === '/demo' },
  {
    to: '/operations',
    label: copy.value.operations,
    match: (path: string) => path === '/operations',
  },
])

const health = computed(() => {
  const states = [api.value.state, rpc.value.state]
  if (states.includes('checking')) return 'checking'
  if (states.every((state) => state === 'online')) return 'online'
  if (states.every((state) => state === 'offline')) return 'offline'
  return 'partial'
})

const roleLabel = computed(() => {
  if (!session.value) return ''
  if (session.value.mode === 'visitor') return copy.value.visitor
  const role = findRole(session.value.role)
  return role ? tr(role.label) : session.value.role
})

watch(
  () => route.fullPath,
  () => (menuOpen.value = false),
)

async function leave(): Promise<void> {
  signOut()
  await router.push('/')
}
</script>

<template>
  <header class="site-header">
    <div class="site-header__inner">
      <RouterLink class="site-header__brand" to="/" aria-label="Lastro">
        <img src="/logo.svg" alt="" width="28" height="28" />
        <span>Lastro</span>
      </RouterLink>

      <button
        type="button"
        class="site-header__toggle"
        :aria-expanded="menuOpen"
        aria-controls="site-nav"
        @click="menuOpen = !menuOpen"
      >
        {{ copy.menu }}
      </button>

      <div id="site-nav" class="site-header__panel" :data-open="menuOpen">
        <nav class="site-header__nav" :aria-label="copy.nav">
          <RouterLink
            v-for="link in links"
            :key="link.to"
            :to="link.to"
            :aria-current="link.match(route.path, route.hash) ? 'page' : undefined"
          >
            {{ link.label }}
          </RouterLink>
        </nav>

        <div class="site-header__tools">
          <span
            class="site-status"
            :data-health="health"
            :title="copy.statusTitle(api.state, rpc.state)"
            data-testid="system-status"
          >
            <span class="site-status__dot" aria-hidden="true" />
            {{ copy[health] }}
          </span>
          <LanguageSwitch />
          <template v-if="session">
            <RouterLink class="site-header__session" to="/painel">
              <span class="site-header__role">{{ roleLabel }}</span>
              <span>{{ copy.workspace }}</span>
            </RouterLink>
            <button type="button" class="site-header__out" data-action="sign-out" @click="leave">
              {{ copy.signOut }}
            </button>
          </template>
          <RouterLink v-else class="site-header__signin" to="/login">{{ copy.signIn }}</RouterLink>
        </div>
      </div>
    </div>
  </header>
</template>

<style scoped>
.site-header {
  position: sticky;
  top: 0;
  z-index: 40;
  border-bottom: 1px solid var(--border);
  background: color-mix(in srgb, var(--surface) 94%, transparent);
  backdrop-filter: saturate(1.2) blur(6px);
}

.site-header__inner {
  display: flex;
  align-items: center;
  gap: 24px;
  width: min(100% - 48px, var(--page-width));
  min-height: var(--header-height);
  margin: 0 auto;
}

.site-header__brand {
  display: inline-flex;
  align-items: center;
  gap: 9px;
  color: var(--text);
  font-size: 17px;
  font-weight: 700;
  letter-spacing: -0.01em;
  text-decoration: none;
}

.site-header__toggle {
  display: none;
}

.site-header__panel {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-width: 0;
}

.site-header__nav {
  display: flex;
  align-items: center;
  gap: 2px;
}

.site-header__nav a {
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  color: var(--muted);
  font-size: 14px;
  font-weight: 500;
  text-decoration: none;
  white-space: nowrap;
}

.site-header__nav a:hover {
  background: var(--raised);
  color: var(--text);
}

.site-header__nav a[aria-current='page'] {
  color: var(--primary);
  font-weight: 600;
  box-shadow: inset 0 -2px 0 var(--primary);
  border-radius: 0;
}

.site-header__tools {
  display: flex;
  align-items: center;
  gap: 10px;
}

.site-status {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 4px 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--muted);
  font-size: 12.5px;
  white-space: nowrap;
}

.site-status__dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--faint);
}

.site-status[data-health='online'] .site-status__dot {
  background: var(--success);
}

.site-status[data-health='partial'] .site-status__dot {
  background: var(--warning);
}

.site-status[data-health='offline'] {
  border-color: var(--danger);
  color: var(--danger);
}

.site-status[data-health='offline'] .site-status__dot {
  background: var(--danger);
}

.site-header__signin,
.site-header__session {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 36px;
  padding: 7px 14px;
  border-radius: var(--radius-sm);
  font-size: 14px;
  font-weight: 600;
  text-decoration: none;
  white-space: nowrap;
}

.site-header__signin {
  background: var(--primary);
  color: var(--on-primary);
}

.site-header__signin:hover {
  background: var(--primary-hover);
}

.site-header__session {
  border: 1px solid var(--border-strong);
  color: var(--text);
}

.site-header__session:hover {
  background: var(--raised);
}

.site-header__role {
  padding: 1px 7px;
  border-radius: 999px;
  background: var(--primary-soft);
  color: var(--primary-hover);
  font-size: 12px;
}

.site-header__out {
  padding: 7px 4px;
  border: 0;
  background: none;
  color: var(--muted);
  font-size: 14px;
  cursor: pointer;
}

.site-header__out:hover {
  color: var(--danger);
}

@media (max-width: 1100px) {
  .site-header__nav a {
    padding: 8px 7px;
    font-size: 13.5px;
  }

  .site-status {
    font-size: 0;
    gap: 0;
    padding: 6px;
  }
}

@media (max-width: 920px) {
  .site-header__inner {
    flex-wrap: wrap;
    gap: 0;
    width: min(100% - 32px, var(--page-width));
  }

  .site-header__brand {
    min-height: var(--header-height);
  }

  .site-header__toggle {
    display: inline-flex;
    align-items: center;
    margin-left: auto;
    padding: 7px 12px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text);
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
  }

  .site-header__panel {
    display: none;
    flex-basis: 100%;
    flex-direction: column;
    align-items: stretch;
    padding: 8px 0 16px;
    border-top: 1px solid var(--border);
  }

  .site-header__panel[data-open='true'] {
    display: flex;
  }

  .site-header__nav {
    flex-direction: column;
    align-items: stretch;
  }

  .site-header__nav a {
    padding: 11px 4px;
    font-size: 15px;
  }

  .site-header__nav a[aria-current='page'] {
    box-shadow: inset 3px 0 0 var(--primary);
    padding-left: 12px;
  }

  .site-header__tools {
    flex-wrap: wrap;
  }

  .site-status {
    gap: 7px;
    padding: 4px 10px;
    font-size: 12.5px;
  }
}
</style>
