<script setup lang="ts">
import { ref } from 'vue'
import ChainField from '../../components/chain/ChainField.vue'
import ChainListBox from '../../components/chain/ChainListBox.vue'
import ChainStageLayout from '../../components/chain/ChainStageLayout.vue'
import PropertyMap from '../../components/chain/PropertyMap.vue'
import { producer, product, routes } from '../../demo/chainHistory'
import { tr, useCopy } from '../../i18n'

const property = producer.property
const showNotice = ref(false)

const copy = useCopy({
  en: {
    kicker: 'STAGE 01 · ORIGIN · PROPERTY',
    record: 'Location, area and registered animals.',
    add: 'Add property',
    lot: 'Lot',
    notice: 'Demo: registering new properties will be available after sign-in.',
    latitude: 'Latitude',
    longitude: 'Longitude',
    size: 'Size',
    farm: 'Farm',
    animals: 'Registered animals',
  },
  pt: {
    kicker: 'ETAPA 01 · ORIGEM · PROPRIEDADE',
    record: 'Localização, área e animais cadastrados.',
    add: 'Adicionar propriedade',
    lot: 'Lote',
    notice: 'Demonstração: o cadastro de novas propriedades estará disponível após o login.',
    latitude: 'Latitude',
    longitude: 'Longitude',
    size: 'Tamanho',
    farm: 'Fazenda',
    animals: 'Animais cadastrados',
  },
})
</script>

<template>
  <ChainStageLayout
    :kicker="copy.kicker"
    :pre-title="property.farm"
    :title="tr(property.name)"
    :crumb="tr(property.name)"
    :parent="{ label: producer.name, to: routes.producer }"
    :record-title="copy.record"
  >
    <template #actions>
      <button class="chain-header__button" type="button" @click="showNotice = true">
        {{ copy.add }}
      </button>
      <span class="property-avatar" :title="producer.name" aria-hidden="true">PS</span>
    </template>

    <template #aside>
      <span>{{ producer.name }}</span>
      <span>{{ copy.lot }} {{ product.lot }}</span>
    </template>

    <p v-if="showNotice" class="property-notice" role="status">
      {{ copy.notice }}
    </p>

    <div class="property-record">
      <div class="property-record__data">
        <ChainField :label="copy.latitude" :value="property.latitude" />
        <ChainField :label="copy.longitude" :value="property.longitude" />
        <ChainField :label="copy.size" :value="property.size" />
        <ChainField :label="copy.farm" :value="property.farm" />
        <ChainListBox :title="copy.animals" :items="producer.animals" wide />
      </div>
      <PropertyMap
        :latitude="property.latitude"
        :longitude="property.longitude"
        :animals="producer.animals.length"
      />
    </div>
  </ChainStageLayout>
</template>

<style scoped>
.property-avatar {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  border: 1px solid var(--border-strong);
  border-radius: 50%;
  background: var(--raised);
  color: var(--text);
  font: 700 12px/1 var(--font-mono);
}

.property-notice {
  grid-column: 1 / -1;
  margin: 0;
  padding: 10px 12px;
  border: 1px solid color-mix(in srgb, var(--warning) 45%, var(--border));
  border-radius: var(--radius-sm);
  color: var(--warning);
  font-size: 13px;
}

.property-record {
  display: grid;
  grid-column: 1 / -1;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
  gap: 18px;
  align-items: start;
}

.property-record__data {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px 18px;
}

@media (max-width: 900px) {
  .property-record {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 480px) {
  .property-record__data {
    grid-template-columns: 1fr;
  }
}
</style>
