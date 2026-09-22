<script setup lang="ts">
import { RouterLink } from 'vue-router'

defineProps<{
  title: string
  to: string
  linkLabel: string
}>()
</script>

<template>
  <footer class="story-page-footer">
    <div>
      <p class="story-page-footer__kicker">NEXT</p>
      <h2>{{ title }}</h2>
    </div>
    <RouterLink class="story-page-footer__link" :to="to">
      {{ linkLabel }}
      <span aria-hidden="true">→</span>
    </RouterLink>
  </footer>
</template>

<style scoped>
.story-page-footer {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 40px;
  padding: 56px 0 72px;
}

.story-page-footer__kicker {
  margin: 0 0 12px;
  color: var(--proof);
  font: 700 10px/1.3 var(--font-mono);
  letter-spacing: .16em;
}

.story-page-footer h2 {
  max-width: 24ch;
  margin: 0;
  font-size: clamp(26px, 2.6vw, 38px);
  line-height: 1.08;
  letter-spacing: -.03em;
}

.story-page-footer__link {
  display: inline-flex;
  gap: 36px;
  align-items: center;
  padding: 14px 16px;
  border: 1px solid color-mix(in srgb, var(--primary) 62%, var(--border));
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--primary) 12%, var(--canvas));
  font: 700 12px/1 var(--font-mono);
  text-decoration: none;
  white-space: nowrap;
}

@media (max-width: 700px) {
  .story-page-footer {
    align-items: flex-start;
    flex-direction: column;
    padding: 48px 0 60px;
  }
}
</style>
