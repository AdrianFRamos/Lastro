<script setup lang="ts">
/**
 * Illustrative property map for the demo: a stylized SVG (no map tiles or external requests)
 * with the property boundary and the registered animals' last readings.
 */
import { useCopy } from '../../i18n'

defineProps<{
  latitude: string
  longitude: string
  /** Number of animal markers drawn inside the boundary (max 6). */
  animals: number
}>()

const copy = useCopy({
  en: {
    label: 'Illustrative map of the property',
    legend: 'Registered animals',
    tag: 'Expansion example · illustrative map',
  },
  pt: {
    label: 'Mapa ilustrativo da propriedade',
    legend: 'Animais cadastrados',
    tag: 'Exemplo de expansão · mapa ilustrativo',
  },
})

const markers = [
  [178, 112],
  [222, 98],
  [236, 146],
  [190, 160],
  [210, 128],
  [164, 138],
] as const
</script>

<template>
  <figure class="property-map">
    <span class="property-map__tag">{{ copy.tag }}</span>
    <svg viewBox="0 0 400 260" role="img" :aria-label="copy.label">
      <rect width="400" height="260" class="property-map__ground" />
      <path class="property-map__field" d="M0 0h120l-20 90H0z" />
      <path class="property-map__field" d="M300 0h100v110l-120-30z" />
      <path class="property-map__field" d="M0 190l110-20 30 90H0z" />
      <path class="property-map__field" d="M290 180l110-10v90H270z" />
      <path class="property-map__river" d="M-10 60C80 80 120 20 200 40s150 60 210 30" />
      <path class="property-map__road" d="M0 210C90 200 150 230 230 205S350 150 400 160" />
      <path class="property-map__road" d="M110 0c10 80-10 160 20 260" />
      <path class="property-map__road property-map__road--minor" d="M130 130h270" />
      <circle class="property-map__boundary" cx="205" cy="130" r="78" />
      <circle
        v-for="([x, y], index) in markers.slice(0, animals)"
        :key="index"
        class="property-map__animal"
        :cx="x"
        :cy="y"
        r="5"
      />
      <path
        class="property-map__pin"
        d="M205 102c-10 0-17 7-17 16 0 12 17 28 17 28s17-16 17-28c0-9-7-16-17-16z"
      />
      <circle cx="205" cy="118" r="5" class="property-map__pin-dot" />
    </svg>
    <figcaption>
      <span>{{ latitude }}, {{ longitude }}</span>
      <span><i class="property-map__legend-dot" aria-hidden="true"></i>{{ copy.legend }}</span>
    </figcaption>
  </figure>
</template>

<style scoped>
.property-map {
  display: grid;
  gap: 8px;
  margin: 0;
}

.property-map__tag {
  justify-self: end;
  padding: 3px 8px;
  border: 1px solid color-mix(in srgb, var(--warning) 45%, var(--border));
  border-radius: 999px;
  color: var(--warning);
  font: 700 10px/1.2 var(--font-mono);
}

.property-map svg {
  display: block;
  width: 100%;
  height: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}

.property-map__ground {
  fill: color-mix(in srgb, var(--raised) 80%, var(--canvas));
}

.property-map__field {
  fill: color-mix(in srgb, var(--success) 10%, var(--raised));
}

.property-map__river {
  fill: none;
  stroke: color-mix(in srgb, var(--chain) 55%, transparent);
  stroke-width: 7;
  stroke-linecap: round;
}

.property-map__road {
  fill: none;
  stroke: color-mix(in srgb, var(--muted) 45%, transparent);
  stroke-width: 4;
}

.property-map__road--minor {
  stroke-width: 2;
  stroke-dasharray: 6 5;
}

.property-map__boundary {
  fill: color-mix(in srgb, var(--proof) 12%, transparent);
  stroke: var(--proof);
  stroke-width: 2;
  stroke-dasharray: 7 5;
}

.property-map__animal {
  fill: var(--warning);
  stroke: var(--canvas);
  stroke-width: 2;
}

.property-map__pin {
  fill: var(--primary);
  stroke: var(--canvas);
  stroke-width: 2;
}

.property-map__pin-dot {
  fill: var(--text);
}

.property-map figcaption {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 6px 16px;
  color: var(--muted);
  font: 600 11px/1.4 var(--font-mono);
}

.property-map__legend-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  margin-right: 6px;
  border-radius: 50%;
  background: var(--warning);
}
</style>
