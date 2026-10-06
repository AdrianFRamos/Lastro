<script setup lang="ts">
import { computed } from 'vue'
import ChainField from '../../components/chain/ChainField.vue'
import ChainListBox from '../../components/chain/ChainListBox.vue'
import ChainStageLayout from '../../components/chain/ChainStageLayout.vue'
import { animal, formatDate, producer, routes } from '../../demo/chainHistory'
import { tr, useCopy } from '../../i18n'

const copy = useCopy({
  en: {
    kicker: 'STAGE 01 · ORIGIN · ANIMAL',
    record: 'Identity, vaccinations and weighings of the source animal.',
    animal: 'Animal hash',
    property: 'Property',
    tag: 'RFID ear tag',
    vaccines: 'Vaccination history',
    weights: 'Weight history',
  },
  pt: {
    kicker: 'ETAPA 01 · ORIGEM · ANIMAL',
    record: 'Identidade, vacinas e pesagens do animal de origem.',
    animal: 'Hash do animal',
    property: 'Propriedade',
    tag: 'Brinco RFID',
    vaccines: 'Histórico de vacinas',
    weights: 'Histórico de pesos',
  },
})

const vaccines = computed(() =>
  animal.vaccines.map((dose) => ({
    label: tr(dose.name),
    note: [formatDate(dose.date), dose.note && tr(dose.note)].filter(Boolean).join(' · '),
  })),
)

const weights = computed(() =>
  animal.weights.map((entry, index) => {
    const previous = animal.weights[index - 1]
    const gain = previous ? ` · +${entry.kg - previous.kg} kg` : ''
    return { label: `${entry.kg} kg`, note: `${formatDate(entry.date)}${gain}` }
  }),
)
</script>

<template>
  <ChainStageLayout
    :kicker="copy.kicker"
    :pre-title="animal.label"
    :title="producer.name"
    :crumb="animal.label"
    :parent="{ label: producer.name, to: routes.producer }"
    :period="producer.period"
    :record-title="copy.record"
  >
    <ChainField :label="copy.animal" :value="animal.hash" hash wide />
    <ChainField
      :label="copy.property"
      :value="`${tr(producer.property.name)} · ${producer.property.farm}`"
      :to="routes.producerProperty"
    />
    <ChainField :label="copy.tag" :value="animal.rfidTag" />
    <ChainListBox :title="copy.vaccines" :items="vaccines" illustrative />
    <ChainListBox :title="copy.weights" :items="weights" illustrative />
  </ChainStageLayout>
</template>
