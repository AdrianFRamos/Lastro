<script setup lang="ts">
defineProps<{ title?: string; description?: string; raised?: boolean }>()
</script>

<template>
  <section class="app-card" :class="{ 'app-card--raised': raised }">
    <header v-if="title || description" class="app-card__header">
      <div>
        <h2 v-if="title" class="app-card__title">{{ title }}</h2>
        <p v-if="description" class="app-card__description">{{ description }}</p>
      </div>
      <slot name="header-action" />
    </header>
    <slot />
  </section>
</template>
