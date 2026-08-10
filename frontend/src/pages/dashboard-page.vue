<template>
  <div class="page dashboard-page">
    <section class="hero-card">
      <div>
        <h2>Test the idea.<br><em>Trade the evidence.</em></h2>
        <p>Build strategies, study market behavior, and move into paper trading from one focused workspace.</p>
        <div class="hero-actions">
          <button class="primary" @click="$emit('navigate', 'experiment')"><Beaker :size="17" /> New experiment</button>
          <button class="secondary" @click="$emit('navigate', 'live')"><Radio :size="17" /> Start paper trading</button>
        </div>
      </div>
      <div class="market-visual" aria-hidden="true">
        <svg viewBox="0 0 420 220" preserveAspectRatio="none">
          <defs><linearGradient id="fill" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#2684ff" stop-opacity=".35"/><stop offset="1" stop-color="#2684ff" stop-opacity="0"/></linearGradient></defs>
          <path d="M0 180 C45 175 58 120 100 135 S157 170 190 105 S245 55 280 80 S340 130 420 25 L420 220 L0 220Z" fill="url(#fill)"/>
          <path d="M0 180 C45 175 58 120 100 135 S157 170 190 105 S245 55 280 80 S340 130 420 25" fill="none" stroke="#5ba1ff" stroke-width="3"/>
        </svg>
        <div class="floating-quote"><span>WORKSPACE</span><strong>{{ loading ? 'Loading…' : loadError ? 'Unavailable' : `${data?.metrics?.symbols || 0} instruments` }}</strong><small>{{ loading ? 'Checking local storage' : loadError ? 'Could not read local storage' : `${format(data?.metrics?.bars)} stored bars` }}</small></div>
      </div>
    </section>

    <div v-if="loadError" class="callout download-plan-error"><TriangleAlert :size="18" /><span>{{ loadError }}</span><button class="secondary" @click="load">Retry</button></div>

    <section class="metric-grid">
      <article v-for="metric in metrics" :key="metric.label" class="metric-card">
        <div class="metric-icon"><component :is="metric.icon" :size="19" /></div>
        <span>{{ metric.label }}</span><strong>{{ metric.value }}</strong><small>{{ metric.note }}</small>
      </article>
    </section>

    <section class="split-grid">
      <article class="panel">
        <div class="panel-header"><div><span class="eyebrow">Latest activity</span><h3>Recent experiments</h3></div><button class="text-button" @click="$emit('navigate', 'results')">View all <ArrowUpRight :size="15" /></button></div>
        <div v-if="loading" class="empty-state" role="status"><span class="spinner" /><p>Loading recent experiments…</p></div>
        <div v-else-if="!loadError && !data?.experiments?.length" class="empty-state"><Beaker/><p>No experiments yet.</p><button class="secondary" @click="$emit('navigate', 'experiment')">Create your first</button></div>
        <button v-for="experiment in data?.experiments" :key="experiment.id" class="activity-row" @click="openExperiment(experiment)">
          <span class="asset-avatar">{{ (experiment.name || 'E').slice(0, 2).toUpperCase() }}</span>
          <span><strong>{{ experiment.name }}</strong><small>{{ time(experiment.started_at) }}</small></span>
          <span class="badge" :class="String(experiment.status).toLowerCase()">{{ experiment.status }}</span>
          <ChevronRight :size="17" />
        </button>
      </article>
      <article class="panel">
        <div class="panel-header"><div><span class="eyebrow">Market data</span><h3>Recently stored</h3></div><button class="text-button" @click="$emit('navigate', 'storage')">Manage <ArrowUpRight :size="15" /></button></div>
        <div v-if="loading" class="empty-state" role="status"><span class="spinner" /><p>Loading stored market data…</p></div>
        <div v-else-if="!loadError && !data?.storage?.length" class="empty-state"><Database/><p>Your local database is empty.</p><button class="secondary" @click="$emit('navigate', 'download')">Download data</button></div>
        <div v-for="row in data?.storage" :key="`${row.symbol}-${row.interval}`" class="activity-row static-row">
          <img v-if="logo(row.symbol, row.instrument_type)" :src="logo(row.symbol, row.instrument_type)" class="symbol-logo" alt="" />
          <span v-else class="asset-avatar">{{ row.symbol?.slice(0, 2) }}</span>
          <span><strong>{{ row.symbol }}</strong><small>{{ row.provider }} · {{ row.interval }}</small></span>
          <strong class="row-value">{{ format(row.n_rows || row.rows) }}</strong>
        </div>
      </article>
    </section>
  </div>
</template>

<script setup>
import { ArrowUpRight, Beaker, ChevronRight, Database, FlaskConical, Radio, Rows3, Shapes, TriangleAlert, WalletCards } from 'lucide-vue-next'
import { computed, onMounted, ref } from 'vue'
import { api } from '../api'
import { instrumentLogoUrl } from '../state'

const props = defineProps({ bootstrap: Object })
const emit = defineEmits(['navigate', 'toast'])
const data = ref(null)
const loading = ref(true)
const loadError = ref('')
const metrics = computed(() => [
  { label: 'Experiments', value: loading.value || loadError.value ? '—' : format(data.value?.metrics?.experiments), note: 'stored locally', icon: FlaskConical },
  { label: 'Instruments', value: loading.value || loadError.value ? '—' : format(data.value?.metrics?.symbols), note: 'ready to analyze', icon: WalletCards },
  { label: 'Market bars', value: loading.value || loadError.value ? '—' : format(data.value?.metrics?.bars), note: 'across all intervals', icon: Rows3 },
  { label: 'Data series', value: loading.value || loadError.value ? '—' : format(data.value?.metrics?.series), note: 'provider feeds', icon: Shapes }
])
function format(value) { return new Intl.NumberFormat('en', { notation: Number(value) > 99999 ? 'compact' : 'standard' }).format(value || 0) }
function time(value) { if (!value) return 'Recently'; return new Date(Number(value) * (Number(value) < 1e12 ? 1000 : 1)).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' }) }
function logo(symbol, type = '') {
  return instrumentLogoUrl(symbol, type, props.bootstrap.display.logokit_api_key)
}
function openExperiment(experiment) {
  sessionStorage.setItem('backtide:result-id', experiment.id)
  emit('navigate', 'results')
}
async function load() {
  loading.value = true
  loadError.value = ''
  try {
    data.value = await api('/api/dashboard')
  } catch (error) {
    data.value = null
    loadError.value = `Could not load workspace data. ${error.message}`
    emit('toast', loadError.value, 'error')
  } finally {
    loading.value = false
  }
}
onMounted(load)
</script>
