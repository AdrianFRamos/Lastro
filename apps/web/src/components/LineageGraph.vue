<template>
  <section class="lineage-graph" aria-labelledby="lineage-graph-title">
    <header class="lineage-graph__header">
      <div>
        <p class="eyebrow">VERIFIED LINEAGE</p>
        <h2 id="lineage-graph-title">Asset transformation graph</h2>
        <p class="app-card__description">
          Parents enter from the left, the facility transformation is in the center, and derived
          assets leave on the right.
        </p>
      </div>
      <div class="lineage-graph__legend" aria-label="Lineage legend">
        <span><i class="lineage-legend-dot lineage-legend-dot--parent" />Input</span>
        <span><i class="lineage-legend-dot lineage-legend-dot--transformation" />Transformation</span>
        <span><i class="lineage-legend-dot lineage-legend-dot--child" />Output</span>
      </div>
    </header>

    <div v-if="!edges.length" class="empty-state">
      No lineage edges were returned for this asset.
    </div>
    <div v-else class="lineage-graph__canvas">
      <svg
        :viewBox="`0 0 ${width} ${height}`"
        role="img"
        aria-label="Graph showing asset parents, transformations, and derived assets"
      >
        <defs>
          <marker id="lineage-arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto">
            <path d="M0,0 L8,4 L0,8 z" class="lineage-arrow" />
          </marker>
        </defs>

        <g v-for="edge in renderedEdges" :key="edge.key">
          <path
            :d="edge.path"
            class="lineage-edge"
            marker-end="url(#lineage-arrow)"
          />
          <text :x="edge.labelX" :y="edge.labelY" class="lineage-edge__label">
            {{ roleLabel(edge.role) }} · {{ formatWeight(edge.weightGrams) }}
          </text>
        </g>

        <g v-for="node in assetNodes" :key="node.key">
          <rect
            :x="node.x - 92"
            :y="node.y - 31"
            width="184"
            height="62"
            rx="10"
            class="lineage-node lineage-node--asset"
          />
          <text :x="node.x" :y="node.y - 4" text-anchor="middle" class="lineage-node__kind">
            {{ node.kind }}
          </text>
          <text :x="node.x" :y="node.y + 15" text-anchor="middle" class="lineage-node__id">
            {{ shortId(node.id) }}
          </text>
        </g>

        <g v-for="node in transformationNodes" :key="node.key">
          <rect
            :x="node.x - 108"
            :y="node.y - 38"
            width="216"
            height="76"
            rx="12"
            class="lineage-node lineage-node--transformation"
          />
          <text :x="node.x" :y="node.y - 10" text-anchor="middle" class="lineage-node__kind">
            TRANSFORMATION
          </text>
          <text :x="node.x" :y="node.y + 11" text-anchor="middle" class="lineage-node__id">
            {{ shortId(node.id) }}
          </text>
          <text :x="node.x" :y="node.y + 27" text-anchor="middle" class="lineage-node__meta">
            {{ node.edgeCount }} edges
          </text>
        </g>
      </svg>
    </div>
    <p v-if="truncated" class="status-message" data-tone="warning">
      The public response was limited to the first 96 edges for rendering. The API response remains
      bounded and can be queried by transformation for deeper inspection.
    </p>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { LineageEdge } from '../api/types'

type AssetNode = {
  key: string
  id: string
  kind: 'INPUT' | 'OUTPUT'
  x: number
  y: number
}

type TransformationNode = {
  key: string
  id: string
  x: number
  y: number
  edgeCount: number
}

type RenderedEdge = {
  key: string
  role: number
  weightGrams: number
  path: string
  labelX: number
  labelY: number
}

const props = defineProps<{ edges: LineageEdge[] }>()
const width = 920
const rowHeight = 150
const visibleEdges = computed(() => props.edges.slice(0, 96))
const truncated = computed(() => props.edges.length > visibleEdges.value.length)
const height = computed(() => Math.max(260, (transformationGroups.value.length + 1) * rowHeight))

const transformationGroups = computed(() => {
  const groups = new Map<string, LineageEdge[]>()
  for (const edge of visibleEdges.value) {
    const current = groups.get(edge.transformationId) ?? []
    current.push(edge)
    groups.set(edge.transformationId, current)
  }
  return [...groups.entries()].map(([id, edges]) => ({ id, edges }))
})

const transformationNodes = computed<TransformationNode[]>(() =>
  transformationGroups.value.map((group, index) => ({
    key: `transformation-${group.id}`,
    id: group.id,
    x: 460,
    y: 120 + index * rowHeight,
    edgeCount: group.edges.length,
  })),
)

const assetNodes = computed<AssetNode[]>(() => {
  const nodes: AssetNode[] = []
  for (const [groupIndex, group] of transformationGroups.value.entries()) {
    const centerY = 120 + groupIndex * rowHeight
    const parents = group.edges.filter((edge) => edge.role === 1)
    const outputs = group.edges.filter((edge) => edge.role !== 1)
    parents.forEach((edge, index) => {
      nodes.push({
        key: `parent-${group.id}-${edge.position}-${index}`,
        id: edge.parentAssetId,
        kind: 'INPUT',
        x: 130,
        y: centerY + (index - (parents.length - 1) / 2) * 58,
      })
    })
    outputs.forEach((edge, index) => {
      nodes.push({
        key: `child-${group.id}-${edge.position}-${index}`,
        id: edge.childAssetId,
        kind: 'OUTPUT',
        x: 790,
        y: centerY + (index - (outputs.length - 1) / 2) * 58,
      })
    })
  }
  return nodes
})

const renderedEdges = computed<RenderedEdge[]>(() => {
  const output: RenderedEdge[] = []
  for (const [groupIndex, group] of transformationGroups.value.entries()) {
    const centerY = 120 + groupIndex * rowHeight
    const parents = group.edges.filter((edge) => edge.role === 1)
    const outputs = group.edges.filter((edge) => edge.role !== 1)
    parents.forEach((edge, index) => {
      const y = centerY + (index - (parents.length - 1) / 2) * 58
      output.push({
        key: `input-edge-${group.id}-${edge.position}-${index}`,
        role: edge.role,
        weightGrams: edge.weightGrams,
        path: `M 222 ${y} C 290 ${y}, 330 ${centerY}, 352 ${centerY}`,
        labelX: 285,
        labelY: y - 7,
      })
    })
    outputs.forEach((edge, index) => {
      const y = centerY + (index - (outputs.length - 1) / 2) * 58
      output.push({
        key: `output-edge-${group.id}-${edge.position}-${index}`,
        role: edge.role,
        weightGrams: edge.weightGrams,
        path: `M 568 ${centerY} C 610 ${centerY}, 650 ${y}, 698 ${y}`,
        labelX: 635,
        labelY: y - 7,
      })
    })
  }
  return output
})

function shortId(value: string): string {
  return `${value.slice(0, 8)}…${value.slice(-8)}`
}

function roleLabel(role: number): string {
  if (role === 1) return 'input'
  if (role === 2) return 'output'
  if (role === 3) return 'byproduct'
  return 'loss'
}

function formatWeight(grams: number): string {
  return `${(grams / 1000).toLocaleString(undefined, { maximumFractionDigits: 3 })} kg`
}
</script>
