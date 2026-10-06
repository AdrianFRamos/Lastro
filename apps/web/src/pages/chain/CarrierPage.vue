<script setup lang="ts">
import ChainField from '../../components/chain/ChainField.vue'
import ChainListBox from '../../components/chain/ChainListBox.vue'
import ChainStageLayout from '../../components/chain/ChainStageLayout.vue'
import { carrier } from '../../demo/chainHistory'
import { tr, useCopy } from '../../i18n'

const copy = useCopy({
  en: {
    kicker: 'STAGE 03 · LOGISTICS',
    record: 'Refrigerated transport between the slaughterhouse and the market.',
    transfer: 'Transfer hash',
    sender: 'Sender',
    recipient: 'Recipient',
    weight: 'Transported weight',
    stops: 'Stops along the way',
    pieces: 'Transported pieces',
  },
  pt: {
    kicker: 'ETAPA 03 · LOGÍSTICA',
    record: 'Transporte refrigerado entre o frigorífico e o mercado.',
    transfer: 'Hash de transferência',
    sender: 'Remetente',
    recipient: 'Destinatário',
    weight: 'Peso transportado',
    stops: 'Paradas no caminho',
    pieces: 'Peças transportadas',
  },
})
</script>

<template>
  <ChainStageLayout
    :kicker="copy.kicker"
    :title="tr(carrier.name)"
    :crumb="tr(carrier.name)"
    :period="carrier.period"
    :record-title="copy.record"
  >
    <ChainField :label="copy.transfer" :value="carrier.transferHash" hash wide />
    <ChainField :label="copy.sender" :value="tr(carrier.sender.label)" :to="carrier.sender.to" />
    <ChainField
      :label="copy.recipient"
      :value="tr(carrier.recipient.label)"
      :to="carrier.recipient.to"
    />
    <ChainField :label="copy.weight" :value="tr(carrier.transportedWeight)" wide />
    <ChainListBox :title="copy.stops" :items="carrier.stops" />
    <ChainListBox :title="copy.pieces" :items="carrier.pieces" />
  </ChainStageLayout>
</template>
