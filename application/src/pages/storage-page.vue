<template>
  <div class="page">
    <section class="page-intro"><div><h2>Stored market data</h2><p>Inspect coverage by symbol, interval and provider, then open it directly in analysis.</p></div><button class="secondary" @click="$emit('navigate', 'download')"><Plus :size="16" /> Add data</button></section>
    <section class="metric-grid three-metrics">
      <article class="metric-card"><div class="metric-icon"><Shapes :size="19" /></div><span>Series</span><strong>{{ loading ? '—' : filtered.length }}</strong><small>matching rows</small></article>
      <article class="metric-card"><div class="metric-icon"><WalletCards :size="19" /></div><span>Symbols</span><strong>{{ loading ? '—' : new Set(filtered.map(row => row.symbol)).size }}</strong><small>unique instruments</small></article>
      <article class="metric-card"><div class="metric-icon"><Rows3 :size="19" /></div><span>Bars</span><strong>{{ loading ? '—' : compact(filtered.reduce((sum, row) => sum + Number(row.n_rows || row.rows || 0), 0)) }}</strong><small>stored observations</small></article>
    </section>
    <section class="panel table-panel">
      <div class="toolbar inline-toolbar">
        <label class="search-box"><Search :size="17"/><input v-model="search" placeholder="Search symbols, providers or intervals…" /></label>
        <button class="danger secondary" :disabled="!selected.size" @click="requestDelete"><Trash2 :size="16"/> Delete {{ selected.size || '' }}</button>
      </div>
      <div v-if="loading" class="empty-state" role="status"><span class="spinner" /><p>Loading stored market data…</p></div>
      <div v-else-if="loadError" class="empty-state error-state"><TriangleAlert/><p>{{ loadError }}</p><button class="secondary" @click="load">Retry</button></div>
      <div v-else class="data-table-wrap">
        <table class="data-table">
          <thead><tr><th><input type="checkbox" :checked="allSelected" @change="toggleAll" /></th><th>Instrument</th><th>Interval</th><th>Provider</th><th>Coverage</th><th class="number">Bars</th><th /></tr></thead>
          <tbody>
            <tr v-for="row in filtered" :key="key(row)">
              <td><input type="checkbox" :checked="selected.has(key(row))" @change="toggle(row)" /></td>
              <td>
                <div class="storage-instrument">
                  <img v-if="logo(row)" :src="logo(row)" class="symbol-logo" alt="" @error="markLogoFailed(row)" />
                  <span v-else class="asset-avatar" aria-hidden="true">{{ row.symbol?.slice(0, 2) }}</span>
                  <span><strong>{{ row.symbol }}</strong><small>{{ row.name || row.instrument_type }}</small></span>
                </div>
              </td>
              <td><span class="badge neutral interval-badge">{{ formatIntervalLabel(row.interval) }}</span></td><td>{{ row.provider }}</td>
              <td><span class="coverage-range">{{ date(row.first_ts || row.earliest_ts) }} <ArrowRight :size="13"/> {{ date(row.last_ts || row.latest_ts) }} <small v-if="coverageDuration(row)">({{ coverageDuration(row) }})</small></span></td>
              <td class="number">{{ compact(row.n_rows || row.rows) }}</td>
              <td><button class="icon-button" aria-label="Open in analysis" @click="openAnalysis(row)"><ChartNoAxesCombined :size="16"/></button></td>
            </tr>
          </tbody>
        </table>
      </div>
      <div v-if="!loading && !loadError && !filtered.length" class="empty-state"><Database/><p>No stored series match this search.</p></div>
    </section>
    <ConfirmationModal
      :open="Boolean(pendingDelete)"
      :title="pendingDelete ? `Delete ${pendingDelete.count} stored series?` : ''"
      message="Are you sure you want to delete every bar in the selected series? This action cannot be undone."
      :busy="deleting"
      @cancel="pendingDelete = null"
      @confirm="destroy"
    />
  </div>
</template>

<script setup>
import { ArrowRight, ChartNoAxesCombined, Database, Plus, Rows3, Search, Shapes, Trash2, TriangleAlert, WalletCards } from 'lucide-vue-next'
import { computed, onActivated, onMounted, ref } from 'vue'
import { api, remove } from '../api'
import ConfirmationModal from '../components/confirmation-modal.vue'
import { formatConfiguredDate, formatDaySpan, formatIntervalLabel, instrumentLogoUrl } from '../state'
const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['navigate', 'toast'])
const rows = ref([])
const loading = ref(true)
const loadError = ref('')
const search = ref('')
const selected = ref(new Set())
const deleting = ref(false)
const pendingDelete = ref(null)
const failedLogos = ref(new Set())
let activatedOnce = false
const filtered = computed(() => rows.value.filter(row => Object.values(row).join(' ').toLowerCase().includes(search.value.toLowerCase())))
const allSelected = computed(() => filtered.value.length && filtered.value.every(row => selected.value.has(key(row))))
const key = row => `${row.symbol}|${row.interval}|${row.provider}`
const logoKey = row => `${row.symbol}|${row.instrument_type}`
const compact = value => new Intl.NumberFormat('en', { notation: Number(value) > 99999 ? 'compact' : 'standard' }).format(Number(value) || 0)
const timestampMs = value => Number(value) * (Number(value) < 1e12 ? 1000 : 1)
function date(value) { return formatConfiguredDate(value, props.bootstrap?.display) }
function coverageDuration(row) {
  const start = timestampMs(row.first_ts || row.earliest_ts)
  const end = timestampMs(row.last_ts || row.latest_ts)
  if (!Number.isFinite(start) || !Number.isFinite(end)) return ''
  const days = Math.max(0, Math.floor(Math.abs(end - start) / 86_400_000))
  return formatDaySpan(days)
}
function logo(row) {
  if (failedLogos.value.has(logoKey(row))) return ''
  return instrumentLogoUrl(row.symbol, row.instrument_type, props.bootstrap?.display?.logokit_api_key)
}
function markLogoFailed(row) {
  failedLogos.value = new Set(failedLogos.value).add(logoKey(row))
}
function toggle(row) { const next = new Set(selected.value); next.has(key(row)) ? next.delete(key(row)) : next.add(key(row)); selected.value = next }
function toggleAll() { selected.value = allSelected.value ? new Set() : new Set(filtered.value.map(key)) }
async function load() {
  loading.value = true
  loadError.value = ''
  try {
    rows.value = await api('/api/storage')
  } catch (error) {
    rows.value = []
    loadError.value = `Could not load stored market data. ${error.message}`
    emit('toast', loadError.value, 'error')
  } finally {
    loading.value = false
  }
}
function openAnalysis(row) {
  sessionStorage.setItem('backtide:analysis-symbols', JSON.stringify([row.symbol]))
  sessionStorage.setItem('backtide:analysis-interval', row.interval)
  emit('navigate', 'analysis')
}
function requestDelete() {
  const series = rows.value.filter(row => selected.value.has(key(row))).map(row => [row.symbol, row.interval, row.provider])
  if (series.length) pendingDelete.value = { count: series.length, series }
}
async function destroy() {
  const target = pendingDelete.value
  if (!target) return
  deleting.value = true
  try {
    const result = await remove('/api/storage', { series: target.series })
    pendingDelete.value = null
    emit('toast', `${compact(result.deleted)} bars deleted.`)
    selected.value = new Set()
    await load()
  }
  catch (error) { emit('toast', error.message, 'error') }
  finally { deleting.value = false }
}
onMounted(load)
onActivated(() => {
  if (activatedOnce) load()
  activatedOnce = true
})
</script>
