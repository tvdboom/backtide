<template>
  <div class="page">
    <section class="page-intro">
      <div><h2>{{ title }}</h2><p>Save built-in presets or bring your own Python implementation.</p></div>
      <button class="primary" @click="openAdd"><Plus :size="16" /> Add {{ singular.toLowerCase() }}</button>
    </section>
    <section class="toolbar panel compact">
      <label class="search-box"><Search :size="17" /><input v-model="search" :placeholder="`Search ${title.toLowerCase()}…`" /></label>
      <div class="segmented small"><button type="button" :class="{ active: filter === 'all' }" @click="filter = 'all'">All</button><button type="button" :class="{ active: filter === 'builtin' }" @click="filter = 'builtin'">Built-in</button><button type="button" :class="{ active: filter === 'custom' }" @click="filter = 'custom'">Custom</button></div>
    </section>
    <section v-if="loading" class="loading-screen"><span class="spinner" /> Loading {{ title.toLowerCase() }}…</section>
    <section v-else-if="loadError" class="panel empty-state large error-state">
      <TriangleAlert :size="30" />
      <h3>Could not load {{ title.toLowerCase() }}</h3>
      <p>{{ loadError }}</p>
      <button class="secondary" type="button" @click="load">Try again</button>
    </section>
    <section v-else-if="!allItems.length" class="panel empty-state large"><Library :size="30" /><h3>{{ isMetric ? `No ${title.toLowerCase()}` : `No saved ${title.toLowerCase()}` }}</h3><p>{{ isMetric ? 'Write a custom implementation to get started.' : 'Add a built-in preset or write a custom implementation.' }}</p></section>
    <section v-else-if="!filtered.length" class="panel empty-state large"><Search :size="30" /><h3>No matching {{ title.toLowerCase() }}</h3><p>Try changing your search or filter.</p></section>
    <section v-else class="library-grid">
      <article v-for="item in filtered" :key="item.name" class="library-card">
        <div class="library-card-top"><div class="metric-icon"><LibraryAssetIcon :kind="assetKind" :builtin="item.builtin" /></div><span class="badge">{{ item.builtin ? 'Built-in' : 'Custom' }}</span><button v-if="!isMetric || !item.builtin" class="icon-button" :aria-label="`Edit ${item.name}`" @click="openEdit(item)"><Pencil :size="16" /></button><button v-if="!isMetric || !item.builtin" class="icon-button danger" :aria-label="`Delete ${item.name}`" @click="pendingDelete = item.name"><Trash2 :size="16" /></button></div>
        <h3>{{ item.name }}</h3><span v-if="!isMetric" class="code-name">{{ item.type }}</span><p>{{ item.description }}</p>
      </article>
    </section>

    <div v-if="editor" class="modal-layer" @mousedown.self="closeEditor">
      <form class="modal panel library-editor" @submit.prevent="save">
        <div class="panel-header"><div><span class="eyebrow">{{ isEditing ? 'Saved reusable asset' : 'New reusable asset' }}</span><h3>{{ isEditing ? 'Edit' : 'Add' }} {{ singular }}</h3></div><button type="button" class="icon-button" aria-label="Close" @click="closeEditor"><X /></button></div>
        <div v-if="!isMetric" class="segmented library-editor-mode"><button type="button" :class="{ active: editor === 'builtin' }" @click="selectEditor('builtin')">Built-in</button><button type="button" :class="{ active: editor === 'custom' }" @click="selectEditor('custom')">Custom Python</button></div>
        <div class="form-grid two library-editor-fields">
          <label v-if="editor === 'builtin'">Name<input v-model="draft.name" required maxlength="80" /></label>
          <div v-else class="custom-source-row wide">
            <label>Name<input v-model="draft.name" required maxlength="80" /></label>
            <label class="file-button upload-code-button" title="Replace the editor contents with a Python file">
              <Upload :size="16" />
              <span>Load Python file</span>
              <input type="file" accept=".py,text/x-python" @change="loadPython" />
            </label>
          </div>
          <label v-if="editor === 'builtin'">Type<select v-model="draft.type" @change="resetParameters"><option v-for="item in catalog.builtin" :key="item.type" :value="item.type">{{ item.name }}</option></select></label>
          <label v-for="parameter in selectedBuiltin?.parameters || []" v-if="editor === 'builtin'" :key="parameter.name">
            {{ parameter.label }}
            <input v-if="parameter.kind === 'number'" v-model.number="draft.params[parameter.name]" type="number" :required="parameter.required" step="any" />
            <span v-else-if="parameter.kind === 'boolean'" class="toggle-label"><span>Enabled</span><input v-model="draft.params[parameter.name]" type="checkbox" class="toggle" /></span>
            <input v-else v-model="draft.params[parameter.name]" :required="parameter.required" />
          </label>
          <label v-if="editor === 'custom'" class="wide">Python source<PythonEditor v-model="draft.code" /></label>
        </div>
        <div v-if="editor === 'builtin' && selectedBuiltin" class="callout library-editor-callout"><Info :size="18" /><span><strong>{{ selectedBuiltin.name }}</strong><br>{{ selectedBuiltin.description }}</span></div>
        <div class="form-footer"><span class="form-spacer"/><button type="button" class="secondary" @click="closeEditor">Cancel</button><button class="primary" :disabled="saving"><span v-if="saving" class="spinner small" /><Save v-else :size="16" /> {{ isEditing ? 'Update' : 'Save' }}</button></div>
      </form>
    </div>
    <ConfirmationModal
      :open="Boolean(pendingDelete)"
      :title="pendingDelete ? `Delete ${pendingDelete}?` : ''"
      :message="`Are you sure you want to delete this ${singular.toLowerCase()}? This action cannot be undone.`"
      :busy="deleting"
      @cancel="pendingDelete = ''"
      @confirm="destroy"
    />
  </div>
</template>

<script setup>
import { Info, Library, Pencil, Plus, Save, Search, Trash2, TriangleAlert, Upload, X } from 'lucide-vue-next'
import { computed, onActivated, onMounted, reactive, ref, watch } from 'vue'
import { api, post, put, remove } from '../api'
import { indicatorCodePlaceholder, metricCodePlaceholder, strategyCodePlaceholder } from '../code-placeholders'
import ConfirmationModal from '../components/confirmation-modal.vue'
import LibraryAssetIcon from '../components/library-asset-icon.vue'
import PythonEditor from '../components/python-editor.vue'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['toast'])
const isStrategy = location.hash.slice(1) === 'strategies'
const isMetric = location.hash.slice(1) === 'metrics'
const title = isStrategy ? 'Strategies' : isMetric ? 'Metrics' : 'Indicators'
const singular = isStrategy ? 'Strategy' : isMetric ? 'Metric' : 'Indicator'
const endpoint = isStrategy ? '/api/strategies' : isMetric ? '/api/metrics' : '/api/indicators'
const initialCatalog = isStrategy ? props.bootstrap.strategies : isMetric ? props.bootstrap.metrics : props.bootstrap.indicators
const assetKind = isStrategy ? 'strategy' : isMetric ? 'metric' : 'indicator'
const dataframeClass = props.bootstrap.display?.dataframe_class || 'pd.DataFrame'
const customCodePlaceholder = isStrategy
  ? strategyCodePlaceholder(dataframeClass)
  : isMetric ? metricCodePlaceholder(dataframeClass) : indicatorCodePlaceholder(dataframeClass)
const catalog = reactive({ builtin: [], saved: [] })
const search = ref('')
const filter = ref('all')
const loading = ref(!initialCatalog)
const loadError = ref('')
const editor = ref('')
const editingName = ref('')
const saving = ref(false)
const deleting = ref(false)
const pendingDelete = ref('')
const draft = reactive({ name: '', type: '', code: '', params: {} })
let activatedOnce = false
const isEditing = computed(() => Boolean(editingName.value))
const allItems = computed(() => isMetric ? [...catalog.builtin, ...catalog.saved] : catalog.saved)
const filtered = computed(() => allItems.value.filter(item => filter.value === 'all' || (filter.value === 'builtin') === item.builtin).filter(item => `${item.name} ${item.type} ${item.description}`.toLowerCase().includes(search.value.toLowerCase())))
const selectedBuiltin = computed(() => catalog.builtin.find(item => item.type === draft.type))

function assignCatalog(value) {
  catalog.builtin = [...(value?.builtin || [])]
  catalog.saved = [...(value?.saved || [])]
  draft.type = catalog.builtin[0]?.type || ''
}

async function load() {
  loading.value = true
  loadError.value = ''
  try {
    assignCatalog(await api(endpoint))
  } catch (error) {
    loadError.value = error.message
    emit('toast', error.message, 'error')
  } finally {
    loading.value = false
  }
}
async function save() {
  if (editor.value === 'custom' && !draft.code.trim()) {
    emit('toast', 'Add Python source before saving.', 'error')
    return
  }
  saving.value = true
  try {
    const payload = { kind: editor.value, ...draft }
    if (isEditing.value) await put(`${endpoint}/${encodeURIComponent(editingName.value)}`, payload)
    else await post(endpoint, payload)
    emit('toast', `${singular} ${isEditing.value ? 'updated' : 'saved'}.`)
    closeEditor()
    await load()
  } catch (error) { emit('toast', error.message, 'error') }
  finally { saving.value = false }
}
function openAdd() {
  editingName.value = ''
  draft.name = ''
  draft.type = catalog.builtin[0]?.type || ''
  draft.code = ''
  draft.params = {}
  editor.value = isMetric ? 'custom' : 'builtin'
  if (!isMetric) resetParameters()
}
function openEdit(item) {
  editingName.value = item.name
  draft.name = item.name
  draft.type = item.type
  draft.code = item.source || ''
  draft.params = { ...(item.params || {}) }
  editor.value = item.builtin ? 'builtin' : 'custom'
}
function closeEditor() {
  editor.value = ''
  editingName.value = ''
  draft.name = ''
  draft.code = ''
}
function resetParameters() {
  draft.params = Object.fromEntries(
    (selectedBuiltin.value?.parameters || []).map(parameter => [parameter.name, parameter.default])
  )
}
function selectEditor(value) {
  editor.value = value
  if (value === 'builtin' && !selectedBuiltin.value) {
    draft.type = catalog.builtin[0]?.type || ''
    resetParameters()
  }
}
async function loadPython(event) {
  const file = event.target.files?.[0]
  if (!file) return
  if (!file.name.toLowerCase().endsWith('.py')) {
    emit('toast', 'Choose a Python (.py) file.', 'error')
    return
  }
  draft.code = await file.text()
  if (!draft.name) draft.name = file.name.slice(0, -3)
}
async function destroy() {
  const name = pendingDelete.value
  if (!name) return
  deleting.value = true
  try {
    await remove(`${endpoint}/${encodeURIComponent(name)}`)
    pendingDelete.value = ''
    emit('toast', `${singular} deleted.`)
    await load()
  }
  catch (error) { emit('toast', error.message, 'error') }
  finally { deleting.value = false }
}
watch(editor, value => {
  if (value === 'custom' && !draft.code.trim()) {
    draft.code = customCodePlaceholder
  }
})
onMounted(() => {
  if (initialCatalog) assignCatalog(initialCatalog)
  load()
})
onActivated(() => {
  if (activatedOnce) load()
  activatedOnce = true
})
</script>
