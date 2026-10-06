<script setup lang="ts">
import ChainField from '../../components/chain/ChainField.vue'
import ChainListBox from '../../components/chain/ChainListBox.vue'
import ChainStageLayout from '../../components/chain/ChainStageLayout.vue'
import { formatDate, routes, slaughterhouse } from '../../demo/chainHistory'
import { tr, useCopy } from '../../i18n'

const copy = useCopy({
  en: {
    kicker: 'STAGE 02 · PROCESSING',
    record: 'From the animal received to the derived cuts.',
    transfer: 'Transfer hash',
    lot: 'Animal lot hash',
    animal: 'Animal hash',
    weight: 'Weight',
    loss: 'Loss',
    slaughtered: 'Slaughter date',
    cut: 'Cutting date',
    pieces: 'Cuts derived from the animal',
  },
  pt: {
    kicker: 'ETAPA 02 · PROCESSAMENTO',
    record: 'Do animal recebido às peças derivadas.',
    transfer: 'Hash de transferência',
    lot: 'Hash do lote dos animais',
    animal: 'Hash do animal',
    weight: 'Peso',
    loss: 'Perda',
    slaughtered: 'Data do abatimento',
    cut: 'Data do corte',
    pieces: 'Peças derivadas do animal',
  },
})
</script>

<template>
  <ChainStageLayout
    :kicker="copy.kicker"
    :title="tr(slaughterhouse.name)"
    :crumb="tr(slaughterhouse.name)"
    :period="slaughterhouse.period"
    :record-title="copy.record"
  >
    <ChainField :label="copy.transfer" :value="slaughterhouse.transferHash" hash wide />
    <ChainField :label="copy.lot" :value="slaughterhouse.lotHash" :to="routes.producer" hash wide />
    <ChainField
      :label="copy.animal"
      :value="slaughterhouse.animalHash"
      :to="routes.producerAnimal"
      hash
      wide
    />
    <ChainField :label="copy.weight" :value="tr(slaughterhouse.weight)" />
    <ChainField :label="copy.loss" :value="tr(slaughterhouse.loss)" />
    <ChainField :label="copy.slaughtered" :value="formatDate(slaughterhouse.slaughteredAt)" />
    <ChainField :label="copy.cut" :value="formatDate(slaughterhouse.cutAt)" />
    <ChainListBox :title="copy.pieces" :items="slaughterhouse.pieces" wide />
  </ChainStageLayout>
</template>
