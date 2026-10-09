<script setup lang="ts">
/**
 * One resource of the demo workspace (e.g. the producer's properties): a table of its records
 * and, when the profile may change them, a form to create or edit a record and a two-step
 * delete. Read-only profiles see the same table without any action.
 */
import { computed, ref, watch } from 'vue'
import { formatDate } from '../../demo/chainHistory'
import { findResource, type DemoRecord, type FieldDef, type ResourceDef } from '../../demo/roles'
import { asText, deleteRecord, listRecords, saveRecord } from '../../demo/workspace'
import { locale, tr, useCopy } from '../../i18n'

const props = defineProps<{
  resource: ResourceDef
  readOnly?: boolean
}>()

const copy = useCopy({
  en: {
    add: 'New',
    edit: 'Edit',
    remove: 'Delete',
    confirm: 'Confirm delete',
    cancel: 'Cancel',
    save: 'Save',
    creating: 'New',
    editing: 'Edit',
    actions: 'Actions',
    empty: 'No records yet.',
    emptyEditable: 'No records yet. Use the button above to add the first one.',
    choose: 'Select…',
    required: 'Fill in the required fields:',
    saved: 'Saved.',
    deleted: 'Deleted.',
    removedRecord: 'Removed record',
    count: (n: number) => (n === 1 ? '1 record' : `${n} records`),
    readOnly: 'Read only',
  },
  pt: {
    add: 'Novo',
    edit: 'Editar',
    remove: 'Excluir',
    confirm: 'Confirmar exclusão',
    cancel: 'Cancelar',
    save: 'Salvar',
    creating: 'Novo registro',
    editing: 'Editar',
    actions: 'Ações',
    empty: 'Nenhum registro ainda.',
    emptyEditable: 'Nenhum registro ainda. Use o botão acima para cadastrar o primeiro.',
    choose: 'Selecione…',
    required: 'Preencha os campos obrigatórios:',
    saved: 'Registro salvo.',
    deleted: 'Registro excluído.',
    removedRecord: 'Registro removido',
    count: (n: number) => (n === 1 ? '1 registro' : `${n} registros`),
    readOnly: 'Somente leitura',
  },
})

const rows = computed(() => listRecords(props.resource.id))

const editingId = ref<string | null>(null)
const formOpen = ref(false)
// Number inputs give numbers back through v-model; everything is stored as text on save.
const draft = ref<Record<string, string | number>>({})
/** Set by the first save attempt; from then on the warning follows what is still empty. */
const attempted = ref(false)
const missing = computed(() =>
  attempted.value
    ? props.resource.fields.filter(
        (field) => field.required && asText(draft.value[field.key]) === '',
      )
    : [],
)
const confirmingId = ref<string | null>(null)
const status = ref('')

watch(
  () => props.resource.id,
  () => closeForm(),
)

function fieldId(field: FieldDef): string {
  return `${props.resource.id.replace('.', '-')}-${field.key}`
}

/** A record is named by the first field of its resource (a code, a name, a plate…). */
function recordTitle(resourceId: string, record: DemoRecord | undefined): string {
  const first = findResource(resourceId)?.fields[0]
  return record && first ? record[first.key] || record.id : copy.value.removedRecord
}

function refOptions(field: FieldDef): DemoRecord[] {
  return field.ref ? listRecords(field.ref) : []
}

function refTitle(field: FieldDef, id: string): string {
  if (!field.ref) return id
  return recordTitle(
    field.ref,
    listRecords(field.ref).find((row) => row.id === id),
  )
}

function formatNumber(value: string, unit?: string): string {
  const number = Number(value)
  if (!Number.isFinite(number)) return value
  const language = locale.value === 'pt' ? 'pt-BR' : 'en-GB'
  if (unit?.startsWith('R$')) {
    const money = number.toLocaleString(language, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    })
    const per = unit.slice(2)
    return `R$ ${money}${per}`
  }
  const formatted = number.toLocaleString(language, { maximumFractionDigits: 6 })
  return unit ? `${formatted} ${unit}` : formatted
}

function display(field: FieldDef, record: DemoRecord): string {
  const value = record[field.key] ?? ''
  if (value === '') return '—'
  switch (field.type) {
    case 'ref':
      return refTitle(field, value)
    case 'select':
      return tr(
        field.options?.find((choice) => choice.value === value)?.label ?? { pt: value, en: value },
      )
    case 'date':
      return formatDate(value)
    case 'number':
      return formatNumber(value, field.unit)
    default:
      return value
  }
}

function openCreate(): void {
  editingId.value = null
  draft.value = Object.fromEntries(props.resource.fields.map((field) => [field.key, '']))
  attempted.value = false
  confirmingId.value = null
  formOpen.value = true
}

function openEdit(record: DemoRecord): void {
  editingId.value = record.id
  draft.value = Object.fromEntries(
    props.resource.fields.map((field) => [field.key, record[field.key] ?? '']),
  )
  attempted.value = false
  confirmingId.value = null
  formOpen.value = true
}

function closeForm(): void {
  formOpen.value = false
  editingId.value = null
  attempted.value = false
  confirmingId.value = null
}

function submit(): void {
  attempted.value = true
  if (missing.value.length > 0) return
  saveRecord(props.resource.id, draft.value, editingId.value ?? undefined)
  status.value = copy.value.saved
  closeForm()
}

function askDelete(record: DemoRecord): void {
  confirmingId.value = record.id
}

function confirmDelete(record: DemoRecord): void {
  deleteRecord(props.resource.id, record.id)
  if (editingId.value === record.id) closeForm()
  confirmingId.value = null
  status.value = copy.value.deleted
}

const formTitle = computed(() => {
  if (!editingId.value) return `${copy.value.creating}: ${tr(props.resource.singular)}`
  const record = rows.value.find((row) => row.id === editingId.value)
  return `${copy.value.editing}: ${recordTitle(props.resource.id, record)}`
})
</script>

<template>
  <section class="resource" :aria-labelledby="`${resource.id}-title`">
    <header class="resource__header">
      <div>
        <h2 :id="`${resource.id}-title`">{{ tr(resource.label) }}</h2>
        <p>{{ tr(resource.description) }}</p>
      </div>
      <div class="resource__tools">
        <span class="resource__count">{{ copy.count(rows.length) }}</span>
        <span v-if="readOnly" class="resource__badge">{{ copy.readOnly }}</span>
        <button
          v-else
          type="button"
          class="resource__button resource__button--primary"
          data-action="create"
          @click="openCreate"
        >
          + {{ copy.add }}: {{ tr(resource.singular) }}
        </button>
      </div>
    </header>

    <form
      v-if="formOpen && !readOnly"
      class="resource__form"
      novalidate
      :aria-label="formTitle"
      @submit.prevent="submit"
    >
      <h3>{{ formTitle }}</h3>
      <div class="resource__fields">
        <label v-for="field in resource.fields" :key="field.key" :for="fieldId(field)">
          <span>
            {{ tr(field.label) }}
            <template v-if="field.unit"> ({{ field.unit }})</template>
            <abbr v-if="field.required" title="*" aria-hidden="true">*</abbr>
          </span>
          <select
            v-if="field.type === 'select' || field.type === 'ref'"
            :id="fieldId(field)"
            v-model="draft[field.key]"
            :required="field.required"
            :aria-invalid="missing.includes(field) || undefined"
          >
            <option value="">{{ copy.choose }}</option>
            <template v-if="field.type === 'select'">
              <option v-for="choice in field.options" :key="choice.value" :value="choice.value">
                {{ tr(choice.label) }}
              </option>
            </template>
            <template v-else>
              <option v-for="target in refOptions(field)" :key="target.id" :value="target.id">
                {{ refTitle(field, target.id) }}
              </option>
            </template>
          </select>
          <input
            v-else
            :id="fieldId(field)"
            v-model="draft[field.key]"
            :type="field.type"
            :step="field.type === 'number' ? 'any' : undefined"
            :required="field.required"
            :aria-invalid="missing.includes(field) || undefined"
          />
        </label>
      </div>
      <p v-if="missing.length > 0" class="resource__error" role="alert">
        {{ copy.required }} {{ missing.map((field) => tr(field.label)).join(', ') }}
      </p>
      <div class="resource__form-actions">
        <button type="button" class="resource__button" @click="closeForm">
          {{ copy.cancel }}
        </button>
        <button type="submit" class="resource__button resource__button--primary">
          {{ copy.save }}
        </button>
      </div>
    </form>

    <p v-if="status" class="sr-only" role="status">{{ status }}</p>

    <div v-if="rows.length > 0" class="resource__table-wrap">
      <table class="resource__table">
        <caption class="sr-only">
          {{
            tr(resource.label)
          }}
        </caption>
        <thead>
          <tr>
            <th v-for="field in resource.fields" :key="field.key" scope="col">
              {{ tr(field.label) }}
            </th>
            <th v-if="!readOnly" scope="col">
              <span class="sr-only">{{ copy.actions }}</span>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="record in rows"
            :key="record.id"
            :class="{ 'is-editing': record.id === editingId }"
          >
            <td v-for="field in resource.fields" :key="field.key">{{ display(field, record) }}</td>
            <td v-if="!readOnly" class="resource__row-actions">
              <template v-if="confirmingId === record.id">
                <button
                  type="button"
                  class="resource__link resource__link--danger"
                  data-action="confirm-delete"
                  @click="confirmDelete(record)"
                >
                  {{ copy.confirm }}
                </button>
                <button type="button" class="resource__link" @click="confirmingId = null">
                  {{ copy.cancel }}
                </button>
              </template>
              <template v-else>
                <button
                  type="button"
                  class="resource__link"
                  data-action="edit"
                  @click="openEdit(record)"
                >
                  {{ copy.edit }}
                </button>
                <button
                  type="button"
                  class="resource__link resource__link--danger"
                  data-action="delete"
                  @click="askDelete(record)"
                >
                  {{ copy.remove }}
                </button>
              </template>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <p v-else class="resource__empty">{{ readOnly ? copy.empty : copy.emptyEditable }}</p>
  </section>
</template>

<style scoped>
.resource {
  display: grid;
  gap: 20px;
  min-width: 0;
  padding: 24px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
}

.resource__header {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.resource__header h2 {
  margin: 0;
  font-size: 24px;
  letter-spacing: -0.02em;
}

.resource__header p {
  margin: 6px 0 0;
  color: var(--muted);
  font-size: 14px;
}

.resource__tools {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
}

.resource__count,
.resource__badge {
  color: var(--muted);
  font: 700 11px/1 var(--font-mono);
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.resource__badge {
  padding: 7px 10px;
  border: 1px solid color-mix(in srgb, var(--chain) 45%, var(--border));
  border-radius: 999px;
  color: var(--chain);
}

.resource__button {
  padding: 11px 16px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--canvas);
  color: var(--text);
  font: 700 12px/1 var(--font-mono);
  letter-spacing: 0.06em;
  text-transform: uppercase;
  cursor: pointer;
}

.resource__button--primary {
  border-color: color-mix(in srgb, var(--primary) 78%, var(--border));
  background: color-mix(in srgb, var(--primary) 26%, var(--canvas));
}

.resource__button:hover,
.resource__button:focus-visible {
  border-color: color-mix(in srgb, var(--primary) 86%, white);
}

.resource__form {
  display: grid;
  gap: 16px;
  padding: 20px;
  border: 1px solid color-mix(in srgb, var(--primary) 45%, var(--border));
  border-radius: var(--radius-md);
  background: var(--raised);
}

.resource__form h3 {
  margin: 0;
  font-size: 16px;
}

.resource__fields {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 14px 16px;
}

.resource__fields label {
  display: grid;
  gap: 6px;
  min-width: 0;
}

.resource__fields label > span {
  color: var(--muted);
  font: 700 10px/1.3 var(--font-mono);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.resource__fields abbr {
  margin-left: 2px;
  color: var(--warning);
  text-decoration: none;
}

.resource__fields input,
.resource__fields select {
  width: 100%;
  min-height: 42px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--canvas);
  color: var(--text);
  font: inherit;
  font-size: 14px;
  color-scheme: dark;
}

.resource__fields input:focus-visible,
.resource__fields select:focus-visible {
  border-color: var(--proof);
  outline: 2px solid color-mix(in srgb, var(--proof) 35%, transparent);
  outline-offset: 1px;
}

.resource__fields [aria-invalid='true'] {
  border-color: var(--warning);
}

.resource__error {
  margin: 0;
  color: var(--warning);
  font-size: 13px;
}

.resource__form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.resource__table-wrap {
  overflow-x: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}

.resource__table {
  width: 100%;
  border-collapse: collapse;
  font-size: 14px;
}

.resource__table th,
.resource__table td {
  padding: 12px 14px;
  border-bottom: 1px solid var(--border);
  text-align: left;
  white-space: nowrap;
}

.resource__table th {
  background: var(--raised);
  color: var(--muted);
  font: 700 10px/1.3 var(--font-mono);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.resource__table tbody tr:last-child td {
  border-bottom: 0;
}

.resource__table tr.is-editing td {
  background: color-mix(in srgb, var(--primary) 10%, transparent);
}

.resource__row-actions {
  text-align: right;
}

.resource__link {
  margin-left: 12px;
  padding: 0;
  border: 0;
  background: none;
  color: var(--proof);
  font: 700 12px/1 var(--font-mono);
  cursor: pointer;
}

.resource__link:hover,
.resource__link:focus-visible {
  text-decoration: underline;
}

.resource__link--danger {
  color: #f87171;
}

.resource__empty {
  margin: 0;
  padding: 24px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-md);
  color: var(--muted);
  text-align: center;
}

@media (max-width: 768px) {
  .resource {
    padding: 18px 16px;
  }

  .resource__form {
    padding: 16px;
  }
}
</style>
