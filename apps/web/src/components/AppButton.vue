<script setup lang="ts">
withDefaults(
  defineProps<{
    disabled?: boolean
    type?: 'button' | 'submit'
    variant?: 'primary' | 'secondary' | 'quiet' | 'danger'
    busy?: boolean
  }>(),
  { disabled: false, type: 'button', variant: 'primary', busy: false },
)
</script>

<template>
  <button
    class="app-button"
    :type="type"
    :disabled="disabled"
    :data-variant="variant"
    :aria-busy="busy ? 'true' : undefined"
  >
    <slot />
  </button>
</template>
