<script setup lang="ts">
import ChainField from '../../components/chain/ChainField.vue'
import ChainListBox from '../../components/chain/ChainListBox.vue'
import ChainStageLayout from '../../components/chain/ChainStageLayout.vue'
import { producer, routes } from '../../demo/chainHistory'
import { tr, useCopy } from '../../i18n'

const copy = useCopy({
  en: {
    kicker: 'STAGE 01 · ORIGIN',
    record: 'Where the animal was born, raised and identified.',
    transfer: 'Transfer hash',
    property: 'Property',
    lot: 'Animal lot hash',
    animals: 'Animals in the lot',
    hardware: 'Hardware used',
  },
  pt: {
    kicker: 'ETAPA 01 · ORIGEM',
    record: 'Onde o animal nasceu, cresceu e foi identificado.',
    transfer: 'Hash de transferência',
    property: 'Propriedade',
    lot: 'Hash do lote dos animais',
    animals: 'Animais do lote',
    hardware: 'Hardwares utilizados',
  },
})
</script>

<template>
  <ChainStageLayout
    :kicker="copy.kicker"
    :title="producer.name"
    :crumb="producer.name"
    :period="producer.period"
    :record-title="copy.record"
  >
    <ChainField :label="copy.transfer" :value="producer.transferHash" hash wide />
    <ChainField
      :label="copy.property"
      :value="`${tr(producer.property.name)} · ${producer.property.farm}`"
      :to="routes.producerProperty"
      wide
    />
    <ChainField :label="copy.lot" :value="producer.lotHash" hash wide />
    <ChainListBox :title="copy.animals" :items="producer.animals" />
    <ChainListBox :title="copy.hardware" :items="producer.hardware" />
  </ChainStageLayout>
</template>
