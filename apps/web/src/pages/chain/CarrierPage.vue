<script setup lang="ts">
/** One transport leg between custodians; the route chooses which leg via the `leg` prop. */
import { computed } from 'vue'
import ChainField from '../../components/chain/ChainField.vue'
import ChainListBox from '../../components/chain/ChainListBox.vue'
import ChainStageLayout from '../../components/chain/ChainStageLayout.vue'
import { carriers, type CarrierLegId } from '../../demo/chainHistory'
import { tr, tx, useCopy } from '../../i18n'

const props = defineProps<{ leg: CarrierLegId }>()
const carrier = computed(() => carriers[props.leg])

const copy = useCopy({
  en: {
    stage: 'STAGE',
    logistics: 'LOGISTICS',
    transfer: 'Transfer hash',
    sender: 'Sender',
    recipient: 'Recipient',
    weight: 'Transported weight',
    stops: 'Stops along the way',
  },
  pt: {
    stage: 'ETAPA',
    logistics: 'LOGÍSTICA',
    transfer: 'Hash de transferência',
    sender: 'Remetente',
    recipient: 'Destinatário',
    weight: 'Peso transportado',
    stops: 'Paradas no caminho',
  },
})

const kicker = computed(
  () =>
    `${copy.value.stage} ${String(carrier.value.step).padStart(2, '0')} · ${copy.value.logistics}`,
)
</script>

<template>
  <ChainStageLayout
    :kicker="kicker"
    :title="tr(carrier.name)"
    :crumb="tr(carrier.name)"
    :period="carrier.period"
    :record-title="tr(carrier.description)"
  >
    <ChainField :label="copy.transfer" :value="carrier.transferHash" hash wide />
    <ChainField :label="copy.sender" :value="tx(carrier.sender.label)" :to="carrier.sender.to" />
    <ChainField
      :label="copy.recipient"
      :value="tx(carrier.recipient.label)"
      :to="carrier.recipient.to"
    />
    <ChainField :label="copy.weight" :value="tr(carrier.transportedWeight)" wide />
    <ChainListBox :title="copy.stops" :items="carrier.stops" />
    <ChainListBox :title="tr(carrier.cargoTitle)" :items="carrier.cargo" />
  </ChainStageLayout>
</template>
