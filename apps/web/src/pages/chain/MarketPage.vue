<script setup lang="ts">
import ChainField from '../../components/chain/ChainField.vue'
import ChainStageLayout from '../../components/chain/ChainStageLayout.vue'
import { market, routes } from '../../demo/chainHistory'
import { tr, useCopy } from '../../i18n'

const copy = useCopy({
  en: {
    kicker: 'STAGE 04 · POINT OF SALE',
    record: 'The picanha received at the market.',
    transfer: 'Transfer hash',
    piece: 'Piece hash',
    animal: 'Animal hash',
    weight: 'Piece weight',
    loss: 'Losses',
  },
  pt: {
    kicker: 'ETAPA 04 · PONTO DE VENDA',
    record: 'Recebimento da picanha no mercado.',
    transfer: 'Hash de transferência',
    piece: 'Hash da peça do animal',
    animal: 'Hash do animal',
    weight: 'Peso da peça',
    loss: 'Perdas',
  },
})
</script>

<template>
  <ChainStageLayout
    :kicker="copy.kicker"
    :title="tr(market.name)"
    :crumb="tr(market.name)"
    :period="market.period"
    :record-title="copy.record"
  >
    <ChainField :label="copy.transfer" :value="market.transferHash" hash wide />
    <ChainField :label="copy.piece" :value="market.pieceHash" hash wide />
    <ChainField
      :label="copy.animal"
      :value="market.animalHash"
      :to="routes.producerAnimal"
      hash
      wide
    />
    <ChainField :label="copy.weight" :value="tr(market.pieceWeight)" />
    <ChainField :label="copy.loss" :value="tr(market.loss)" />
  </ChainStageLayout>
</template>
