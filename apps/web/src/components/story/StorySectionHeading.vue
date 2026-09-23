<script setup lang="ts">
defineProps<{
  kicker: string
  title: string
  description?: string
  headingId?: string
}>()
</script>

<template>
  <div class="story-section-heading">
    <p class="story-section-heading__kicker">{{ kicker }}</p>
    <h2 :id="headingId">{{ title }}</h2>
    <p v-if="description" class="story-section-heading__description">{{ description }}</p>
  </div>
</template>

<style scoped>
.story-section-heading {
  display: grid;
  grid-template-columns: minmax(170px, 0.38fr) minmax(0, 0.9fr) minmax(280px, 0.58fr);
  gap: 42px;
  align-items: start;
  margin-bottom: 38px;
}

.story-section-heading__kicker {
  margin: 0;
  color: var(--proof);
  font: 700 10px/1.3 var(--font-mono);
  letter-spacing: 0.16em;
}

.story-section-heading h2 {
  max-width: 22ch;
  margin: 0;
  font-size: clamp(30px, 2.7vw, 42px);
  font-weight: 560;
  line-height: 1.05;
  letter-spacing: -0.035em;
}

.story-section-heading__description {
  margin: 0;
  color: var(--muted);
  font-size: 14px;
  line-height: 1.65;
}

@media (max-width: 1000px) {
  .story-section-heading {
    grid-template-columns: 1fr 1fr;
  }

  .story-section-heading__description {
    grid-column: 2;
  }
}

@media (max-width: 700px) {
  .story-section-heading {
    grid-template-columns: 1fr;
    gap: 6px;
    margin-bottom: 26px;
  }

  .story-section-heading__description {
    grid-column: auto;
    margin-top: 8px;
  }
}
</style>
