<script setup lang="ts">
import ChainField from '../../components/chain/ChainField.vue'
import ChainListBox from '../../components/chain/ChainListBox.vue'
import ChainStageLayout from '../../components/chain/ChainStageLayout.vue'
import { exporter } from '../../demo/chainHistory'
import { tr, useCopy } from '../../i18n'

const copy = useCopy({
  en: {
    kicker: 'STAGE 05 · EXPORT',
    record: 'From the cuts received to the container shipped abroad.',
    transfer: 'Transfer hash',
    container: 'Container',
    origin: 'Port of loading',
    destination: 'Port of discharge',
    temperature: 'Cold chain',
    documents: 'Export documents',
    pieces: 'Exported pieces',
  },
  pt: {
    kicker: 'ETAPA 05 · EXPORTAÇÃO',
    record: 'Das peças recebidas ao contêiner embarcado para o exterior.',
    transfer: 'Hash de transferência',
    container: 'Contêiner',
    origin: 'Porto de embarque',
    destination: 'Porto de destino',
    temperature: 'Cadeia de frio',
    documents: 'Documentos de exportação',
    pieces: 'Peças exportadas',
  },
})
</script>

<template>
  <ChainStageLayout
    :kicker="copy.kicker"
    :title="tr(exporter.name)"
    :crumb="tr(exporter.name)"
    :period="exporter.period"
    :record-title="copy.record"
  >
    <ChainField :label="copy.transfer" :value="exporter.transferHash" hash wide />
    <ChainField :label="copy.container" :value="exporter.container" />
    <ChainField :label="copy.origin" :value="tr(exporter.originPort)" />
    <ChainField :label="copy.destination" :value="tr(exporter.destinationPort)" />
    <ChainField :label="copy.temperature" :value="tr(exporter.temperature)" wide />
    <ChainListBox :title="copy.documents" :items="exporter.documents" />
    <ChainListBox :title="copy.pieces" :items="exporter.pieces" />
  </ChainStageLayout>
</template>
