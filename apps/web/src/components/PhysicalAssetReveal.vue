<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

withDefaults(defineProps<{ src?: string }>(), {
  src: '/lastro.png',
})

const rootRef = ref<HTMLElement | null>(null)
let parent: HTMLElement | null = null
let raf = 0
let active = false
let currentX = 0
let currentY = 0
let targetX = 0
let targetY = 0
let reducedMotionQuery: MediaQueryList | null = null
let finePointerQuery: MediaQueryList | null = null
let listenersAttached = false
let boundsLeft = 0
let boundsTop = 0

function writePosition(): void {
  const root = rootRef.value
  if (!root) return
  root.style.setProperty('--reveal-x', `${currentX}px`)
  root.style.setProperty('--reveal-y', `${currentY}px`)
}

function render(): void {
  const root = rootRef.value
  if (!root) return

  currentX += (targetX - currentX) * 0.18
  currentY += (targetY - currentY) * 0.18
  writePosition()

  if (Math.abs(targetX - currentX) > 0.25 || Math.abs(targetY - currentY) > 0.25) {
    raf = requestAnimationFrame(render)
  } else {
    currentX = targetX
    currentY = targetY
    writePosition()
    raf = 0
  }
}

function ensureFrame(): void {
  if (!raf) raf = requestAnimationFrame(render)
}

function updateBounds(): void {
  if (!parent) return
  const rect = parent.getBoundingClientRect()
  boundsLeft = rect.left
  boundsTop = rect.top
}

function setTargetFromPointer(event: PointerEvent, snap = false): void {
  targetX = event.clientX - boundsLeft
  targetY = event.clientY - boundsTop

  if (snap) {
    currentX = targetX
    currentY = targetY
    writePosition()
  }
}

function setActive(next: boolean): void {
  active = next
  if (rootRef.value) rootRef.value.dataset.active = active ? 'true' : 'false'
}

function onPointerMove(event: PointerEvent): void {
  if (event.pointerType === 'touch') return
  setTargetFromPointer(event)
  setActive(true)
  ensureFrame()
}

function onPointerEnter(event: PointerEvent): void {
  if (event.pointerType === 'touch') return
  updateBounds()
  setTargetFromPointer(event, true)
  setActive(true)
}

function onPointerLeave(): void {
  setActive(false)
}

function attachListeners(): void {
  if (!parent || listenersAttached) return
  updateBounds()
  parent.addEventListener('pointermove', onPointerMove, { passive: true })
  parent.addEventListener('pointerenter', onPointerEnter, { passive: true })
  parent.addEventListener('pointerleave', onPointerLeave, { passive: true })
  listenersAttached = true
}

function detachListeners(): void {
  if (raf) {
    cancelAnimationFrame(raf)
    raf = 0
  }
  if (!parent || !listenersAttached) {
    setActive(false)
    return
  }
  parent.removeEventListener('pointermove', onPointerMove)
  parent.removeEventListener('pointerenter', onPointerEnter)
  parent.removeEventListener('pointerleave', onPointerLeave)
  listenersAttached = false
  setActive(false)
}

function syncInteractivity(): void {
  const shouldAnimate = !reducedMotionQuery?.matches && finePointerQuery?.matches
  if (shouldAnimate) attachListeners()
  else detachListeners()
}

onMounted(() => {
  const root = rootRef.value
  parent = root?.parentElement ?? null
  if (!root || !parent) return

  const rect = parent.getBoundingClientRect()
  boundsLeft = rect.left
  boundsTop = rect.top
  currentX = targetX = rect.width * 0.74
  currentY = targetY = rect.height * 0.43
  writePosition()

  reducedMotionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
  finePointerQuery = window.matchMedia('(pointer: fine)')
  reducedMotionQuery.addEventListener('change', syncInteractivity)
  finePointerQuery.addEventListener('change', syncInteractivity)
  syncInteractivity()
})

onBeforeUnmount(() => {
  if (raf) cancelAnimationFrame(raf)
  detachListeners()
  reducedMotionQuery?.removeEventListener('change', syncInteractivity)
  finePointerQuery?.removeEventListener('change', syncInteractivity)
  reducedMotionQuery = null
  finePointerQuery = null
  parent = null
})
</script>

<template>
  <div ref="rootRef" class="reveal" data-active="false" aria-hidden="true">
    <div class="reveal__image-stage">
      <img class="reveal__image reveal__image--base" :src="src" alt="" draggable="false" />
      <div class="reveal__illuminated">
        <img class="reveal__image reveal__image--lit" :src="src" alt="" draggable="false" />
        <div class="reveal__tint" />
      </div>
    </div>
    <div class="reveal__shade" />
    <div class="reveal__grid" />
  </div>
</template>

<style scoped>
.reveal {
  --reveal-x: 74%;
  --reveal-y: 43%;
  position: absolute;
  inset: 0;
  overflow: hidden;
  pointer-events: none;
  user-select: none;
}

.reveal__image-stage {
  position: absolute;
  inset: 0;
  overflow: hidden;
  opacity: 1;
}

.reveal__image {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: right 45%;
}

.reveal__image--base {
  filter: grayscale(0.5) saturate(0.55) contrast(1.06) brightness(0.52);
  opacity: 0.88;
}

.reveal__illuminated {
  position: absolute;
  inset: 0;
  opacity: 0;
  transition: opacity 180ms ease;
  -webkit-mask-image: radial-gradient(circle 28rem at var(--reveal-x) var(--reveal-y), #000 0%, #000 28%, rgb(0 0 0 / 0.85) 52%, rgb(0 0 0 / 0.3) 76%, transparent 100%);
  mask-image: radial-gradient(circle 28rem at var(--reveal-x) var(--reveal-y), #000 0%, #000 28%, rgb(0 0 0 / 0.85) 52%, rgb(0 0 0 / 0.3) 76%, transparent 100%);
}

.reveal[data-active='true'] .reveal__illuminated {
  opacity: 1;
}

.reveal__image--lit {
  filter: contrast(1.08) brightness(0.95) saturate(1.05);
}

.reveal__tint {
  position: absolute;
  inset: 0;
  background: radial-gradient(circle 24rem at var(--reveal-x) var(--reveal-y), color-mix(in srgb, var(--proof) 14%, transparent) 0%, transparent 75%);
  mix-blend-mode: screen;
  opacity: 0.7;
}

.reveal__shade {
  position: absolute;
  inset: 0;
  background:
    linear-gradient(90deg, var(--canvas) 0%, rgb(11 15 20 / 0.98) 28%, rgb(11 15 20 / 0.75) 48%, rgb(11 15 20 / 0.15) 75%, transparent 100%),
    linear-gradient(180deg, rgb(11 15 20 / 0.35) 0%, transparent 18%, transparent 78%, var(--canvas) 100%);
}

.reveal__grid {
  position: absolute;
  inset: 0;
  background-image:
    linear-gradient(to right, color-mix(in srgb, var(--border) 18%, transparent) 1px, transparent 1px),
    linear-gradient(to bottom, color-mix(in srgb, var(--border) 18%, transparent) 1px, transparent 1px);
  background-size: 4rem 4rem;
  -webkit-mask-image: linear-gradient(90deg, transparent 0%, transparent 35%, #000 75%, transparent 100%);
  mask-image: linear-gradient(90deg, transparent 0%, transparent 35%, #000 75%, transparent 100%);
  opacity: 0.14;
}

@media (max-width: 70rem) {
  .reveal__image {
    object-position: right 45%;
  }
}

@media (max-width: 48rem), (pointer: coarse) {
  .reveal__image-stage {
    inset: 38% 0 0;
    clip-path: none;
    opacity: 0.7;
  }

  .reveal__image {
    inset: 0;
    width: 100%;
    height: 100%;
    object-position: 64% 50%;
    transform: none;
  }

  .reveal__image--base {
    filter: grayscale(0.45) saturate(0.62) contrast(1.05) brightness(0.38);
  }

  .reveal__illuminated {
    display: none;
  }

  .reveal__shade {
    background:
      linear-gradient(180deg, var(--canvas) 0%, rgb(11 15 20 / 0.94) 26%, rgb(11 15 20 / 0.45) 58%, var(--canvas) 100%),
      linear-gradient(90deg, rgb(11 15 20 / 0.72), transparent 72%);
  }

  .reveal__grid {
    opacity: 0.08;
  }
}

@media (prefers-reduced-motion: reduce) {
  .reveal__image--base {
    filter: grayscale(0.38) saturate(0.7) contrast(1.05) brightness(0.43);
  }

  .reveal__illuminated {
    display: none;
  }
}
</style>
