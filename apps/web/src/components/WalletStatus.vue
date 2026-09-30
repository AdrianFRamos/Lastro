<script setup lang="ts">
defineProps<{
  address: string | null
  connected: boolean
}>()
</script>

<template>
  <div class="wallet-status" :data-connected="connected ? 'true' : 'false'" aria-live="polite">
    <span class="wallet-status__dot" aria-hidden="true" />
    <span class="wallet-status__text">
      <span>{{ connected ? 'Connected' : 'Disconnected' }}</span>
      <code v-if="address" :title="address">{{ address }}</code>
    </span>
  </div>
</template>
