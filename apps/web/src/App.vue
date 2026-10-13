<!-- Root shell: global header and footer around the routed page; no domain state lives here. -->
<template>
  <div class="app-shell">
    <a class="skip-link" href="#conteudo">{{ skip }}</a>
    <AppHeader />
    <div id="conteudo" class="app-shell__main" tabindex="-1">
      <RouterView />
    </div>
    <AppFooter />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { RouterView } from 'vue-router'
import AppFooter from './components/shell/AppFooter.vue'
import AppHeader from './components/shell/AppHeader.vue'
import { locale } from './i18n'

const skip = computed(() => (locale.value === 'pt' ? 'Pular para o conteúdo' : 'Skip to content'))
</script>
